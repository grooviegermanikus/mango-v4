mod mango;

use clap::{Args, Parser, Subcommand};
use mango_v4_client::{
    keypair_from_cli, pubkey_from_cli, Client, JupiterSwapMode, MangoClient,
    TransactionBuilderConfig,
};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use chrono::Utc;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::keypair;
use anchor_client::Cluster;
use solana_sdk::signature::Signer;
use fixed::FixedI128;
use fixed::types::extra::U48;
use fixed::types::I80F48;
use mango_v4::state::{PerpMarket, PlaceOrderType, QUOTE_DECIMALS, Side};
use crate::mango::{MINT_ADDRESS_ETH, MINT_ADDRESS_USDC};


#[derive(Parser, Debug)]
#[clap()]
struct CliDotenv {
    // When --dotenv <file> is passed, read the specified dotenv file before parsing args
    #[clap(long)]
    dotenv: std::path::PathBuf,

    remaining_args: Vec<std::ffi::OsString>,
}

#[derive(Parser, Debug, Clone)]
#[clap()]
struct Cli {
    #[clap(short, long, env)]
    rpc_url: String,

    #[clap(short, long, env)]
    mango_account: Pubkey,

    #[clap(short, long, env)]
    owner: String,

    // #[clap(subcommand)]
    // command: Command,
}

#[derive(Args, Debug, Clone)]
struct Rpc {
    #[clap(short, long, default_value = "m")]
    url: String,

    #[clap(short, long, default_value = "")]
    fee_payer: String,
}

#[derive(Args, Debug, Clone)]
struct CreateAccount {
    #[clap(short, long)]
    group: String,

    /// also pays for everything
    #[clap(short, long)]
    owner: String,

    #[clap(short, long)]
    account_num: Option<u32>,

    #[clap(short, long, default_value = "")]
    name: String,

    #[clap(flatten)]
    rpc: Rpc,
}

#[derive(Args, Debug, Clone)]
struct Deposit {
    #[clap(long)]
    account: String,

    /// also pays for everything
    #[clap(short, long)]
    owner: String,

    #[clap(short, long)]
    mint: String,

    #[clap(short, long)]
    amount: u64,

    #[clap(flatten)]
    rpc: Rpc,
}

#[derive(Args, Debug, Clone)]
struct JupiterSwap {
    #[clap(long)]
    account: String,

    /// also pays for everything
    #[clap(short, long)]
    owner: String,

    #[clap(long)]
    input_mint: String,

    #[clap(long)]
    output_mint: String,

    #[clap(short, long)]
    amount: u64,

    #[clap(short, long)]
    slippage_bps: u64,

    #[clap(flatten)]
    rpc: Rpc,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    CreateAccount(CreateAccount),
    Deposit(Deposit),
    JupiterSwap(JupiterSwap),
    GroupAddress {
        #[clap(short, long)]
        creator: String,

        #[clap(short, long, default_value = "0")]
        num: u32,
    },
    MangoAccountAddress {
        #[clap(short, long)]
        group: String,

        #[clap(short, long)]
        owner: String,

        #[clap(short, long, default_value = "0")]
        num: u32,
    },
}

impl Rpc {
    fn client(&self, override_fee_payer: Option<&str>) -> anyhow::Result<Client> {
        let fee_payer = keypair_from_cli(override_fee_payer.unwrap_or(&self.fee_payer));
        Ok(Client::new(
            anchor_client::Cluster::from_str(&self.url)?,
            solana_sdk::commitment_config::CommitmentConfig::confirmed(),
            Arc::new(fee_payer),
            None,
            TransactionBuilderConfig {
                prioritization_micro_lamports: Some(5),
            },
        ))
    }
}



#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );

    let args = if let Ok(cli_dotenv) = CliDotenv::try_parse() {
        dotenv::from_path(cli_dotenv.dotenv)?;
        cli_dotenv.remaining_args
    } else {
        dotenv::dotenv().ok();
        std::env::args_os().collect()
    };

    let cli = Cli::parse_from(args);


    let rpc_url = cli.rpc_url;
    let ws_url = rpc_url.replace("https", "wss");

    // from app mango -> "Accounts"
    // https://mango.devnet.rpcpool.com

    // use private key (solana-keygen)
    let owner: Arc<Keypair> = Arc::new(keypair_from_cli(cli.owner.as_str()));
    println!("owner: {}", owner.pubkey());

    // TODO need two
    let commitment = CommitmentConfig::processed();


    let cluster = Cluster::Custom(rpc_url, ws_url);

    let mango_client = Arc::new(
        MangoClient::new_for_existing_account(
            Client::new(
                cluster,
                commitment,
                owner.clone(),
                Some(Duration::from_secs(5)),
                TransactionBuilderConfig {
                    prioritization_micro_lamports: Some(1),
                },
            ),
            cli.mango_account,
            owner.clone(),
        ).await?);

    let x = mango_client.get_oracle_price("ETH (Portal)");
    println!("oracle price: {:?}", x.await?);


    let market_index = mango_client.context.perp_market_indexes_by_name.get("ETH-PERP").unwrap();
    let perp_market = mango_client.context.perp_markets.get(market_index).unwrap().market.clone();


    // let price = I80F48::from_num(0.01); // min
    // let price_lots = perp_market.native_price_to_lot(price);

    // must be unique
    let client_order_id = Utc::now().timestamp_micros();


    let order_size_lots = native_amount_to_lot(&perp_market, 0.0001);
    println!("order size buy: {}", order_size_lots);

    let sig = mango_client.perp_place_order(
        market_index.clone(),
        Side::Bid, 0 /* ignore price */,
        order_size_lots,
        quote_amount_to_lot(&perp_market, 100.00),
        client_order_id as u64,
        PlaceOrderType::Market,
        false,
        0,
        64 // max num orders to be skipped based on expiry information in the orderbook
    ).await;

    println!("sig buy: {:?}", sig);

    // fails for delegate account
    // let order_size_sell = native_amount(&perp_market, 0.0001);
    // println!("order size sell: {:?}", order_size_sell);
    // let sig_sell = mango_client.jupiter_swap(
    //     Pubkey::from_str(MINT_ADDRESS_ETH).unwrap(),
    //     Pubkey::from_str(MINT_ADDRESS_USDC).unwrap(),
    //     order_size_sell,
    //     10, // TODO 0.1%, 100=1% make configurable
    //     JupiterSwapMode::ExactIn
    // ).await;
    //
    // println!("sig sell: {:?}", sig_sell);

   Ok(())

}

impl LotConversion for PerpMarket {
    fn get_base_decimals(&self) -> u32 {
        self.base_decimals.into()
    }

    fn get_base_lot_size(&self) -> i64 {
        self.base_lot_size
    }

    fn get_quote_lot_size(&self) -> i64 {
        self.quote_lot_size
    }
}

trait LotConversion {
    fn get_base_decimals(&self) -> u32;
    fn get_base_lot_size(&self) -> i64;
    fn get_quote_lot_size(&self) -> i64;

}

fn native_amount_to_lot(lot_conf: &dyn LotConversion, amount: f64) -> i64 {
    // base_decimals=6
    // 0.0001 in 1e6(decimals) = 100 = 1 lot
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(lot_conf.get_base_decimals()))
        / I80F48::from_num(lot_conf.get_base_lot_size());

    exact.to_num::<f64>().round() as i64
}

fn native_amount(lot_conf: &dyn LotConversion, amount: f64) -> u64 {
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(lot_conf.get_base_decimals()));

    exact.to_num::<f64>().round() as u64
}


fn quote_amount_to_lot(lot_conf: &dyn LotConversion, amount: f64) -> i64 {
    // quote_decimals always 6
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(QUOTE_DECIMALS as u32))
        / I80F48::from_num(lot_conf.get_quote_lot_size());

    exact.to_num::<f64>().round() as i64
}


fn quantity_to_lot(perp_market: PerpMarket, amount: f64) -> I80F48 {
    // base_decimals=6
    // 0.0001 in 1e6(decimals) = 100 = 1 lot
    let order_size = I80F48::from_num(amount);

    order_size * I80F48::from_num(10u64.pow(perp_market.base_decimals.into()))
        / I80F48::from_num(perp_market.base_lot_size)
}

mod test {
    use crate::quantity_to_lot;

    #[test]
    fn convert_quantity_eth_perp() {

        // quantity_to_lot()

    }
}
