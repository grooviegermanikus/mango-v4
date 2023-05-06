mod mango;
mod services;
mod coordinator;
mod numerics;

use std::future::Future;
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
use futures::TryFutureExt;
use solana_sdk::commitment_config::CommitmentConfig;
use solana_sdk::signature::Keypair;
use solana_sdk::signer::keypair;
use anchor_client::Cluster;
use solana_sdk::signature::Signer;
use fixed::FixedI128;
use fixed::types::extra::U48;
use fixed::types::I80F48;
use mango_v4::state::{PerpMarket, PerpMarketIndex, PlaceOrderType, QUOTE_DECIMALS, Side};
use crate::mango::{MINT_ADDRESS_ETH, MINT_ADDRESS_USDC};
use crate::numerics::{native_amount, native_amount_to_lot, quote_amount_to_lot};

#[derive(Parser, Debug, Clone)]
#[clap()]
struct Cli {

    // e.g. https://mango.devnet.rpcpool.com
    #[clap(short, long, env)]
    rpc_url: String,

    // from app mango -> "Accounts"
    #[clap(short, long, env)]
    mango_account: Pubkey,

    // path to json array with private key
    #[clap(short, long, env)]
    owner: String,

    // #[clap(subcommand)]
    // command: Command,
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, "info"),
    );


    let cli = Cli::parse_from(std::env::args_os());

    let rpc_url = cli.rpc_url;
    let ws_url = rpc_url.replace("https", "wss");

    // use private key (solana-keygen)
    let owner: Arc<Keypair> = Arc::new(keypair_from_cli(cli.owner.as_str()));

    let cluster = Cluster::Custom(rpc_url, ws_url);

    let mango_client = Arc::new(
        MangoClient::new_for_existing_account(
            Client::new(
                cluster,
                // TODO need two (ask Max)
                CommitmentConfig::processed(),
                owner.clone(),
                Some(Duration::from_secs(5)),
                TransactionBuilderConfig {
                    prioritization_micro_lamports: Some(1),
                },
            ),
            cli.mango_account,
            owner.clone(),
        ).await?);

    // let x = mango_client.get_oracle_price("ETH (Portal)");
    // println!("oracle price: {:?}", x.await?);


    // TODO make it smarter
    let coordinator_thread = tokio::spawn(coordinator::run_coordinator_service());

    // buy_asset(mango_client.clone()).await;
    // sell_asset(mango_client.clone()).await;

    coordinator_thread.await?;

    Ok(())
}

async fn buy_asset(mango_client: Arc<MangoClient>) {
    // must be unique
    let client_order_id = Utc::now().timestamp_micros();

    let market_index = mango_client.context.perp_market_indexes_by_name.get("ETH-PERP").unwrap();
    let perp_market = mango_client.context.perp_markets.get(market_index).unwrap().market.clone();

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

    // println!("sig buy: {:?}", sig);
}

// fails ATM due to delegate account
async fn sell_asset(mango_client: Arc<MangoClient>) {
    let market_index = mango_client.context.perp_market_indexes_by_name.get("ETH-PERP").unwrap();
    let perp_market = mango_client.context.perp_markets.get(market_index).unwrap().market.clone();

    let order_size_sell = native_amount(&perp_market, 0.0001);
    println!("order size sell: {:?}", order_size_sell);
    let sig_sell = mango_client.jupiter_swap(
        Pubkey::from_str(MINT_ADDRESS_ETH).unwrap(),
        Pubkey::from_str(MINT_ADDRESS_USDC).unwrap(),
        order_size_sell,
        10, // TODO 0.1%, 100=1% make configurable
        JupiterSwapMode::ExactIn
    ).await;

    println!("sig sell: {:?}", sig_sell);
}
