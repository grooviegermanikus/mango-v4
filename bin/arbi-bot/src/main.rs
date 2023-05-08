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
use std::thread;
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
use crate::services::perp_orders::buy_asset;

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
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV,
                 "info,arbi_bot=debug"),
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
    let coordinator_thread = tokio::spawn(coordinator::run_coordinator_service(mango_client.clone()));

    // buy_asset(mango_client.clone()).await;
    // sell_asset(mango_client.clone()).await;

    // mango_client.mango_account().await.unwrap().

    coordinator_thread.await?;

    Ok(())
}

