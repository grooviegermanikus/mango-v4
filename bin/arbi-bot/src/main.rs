mod mango;
mod services;
mod coordinator;
mod numerics;

use solana_rpc::rpc_pubsub::{RpcSolPubSub, RpcSolPubSubClient};
use std::future::Future;
use std::rc::Rc;
use clap::{Args, Parser, Subcommand};
use mango_v4_client::{keypair_from_cli, pubkey_from_cli, Client, JupiterSwapMode, MangoClient, TransactionBuilderConfig, AnyhowWrap};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::thread::sleep;
use std::time::{Duration, Instant};
use chrono::Utc;
use futures::future::join_all;
use futures::TryFutureExt;
use jsonrpc_core_client::transports::ws;
use jsonrpc_core_client::TypedSubscriptionStream;
use solana_client::rpc_config::RpcSignatureSubscribeConfig;
use solana_client::rpc_response::{Response, RpcSignatureResult};
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
use crate::services::blockhash::start_blockhash_service;
use crate::services::perp_orders::{perp_bid_asset, perp_ask_asset};
use crate::services::swap_orders::swap_buy_asset;
use crate::services::transactions;

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
                 "info,arbi_bot=trace"),
    );


    let cli = Cli::parse_from(std::env::args_os());

    let rpc_url = cli.rpc_url;
    let ws_url = rpc_url.replace("https", "wss");

    // use private key (solana-keygen)
    let owner: Arc<Keypair> = Arc::new(keypair_from_cli(cli.owner.as_str()));

    let cluster = Cluster::Custom(rpc_url.clone(), ws_url.clone());

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


    // let coordinator_thread = tokio::spawn(coordinator::run_coordinator_service(mango_client.clone()));
    // coordinator_thread.await?;

    // play with confirmation
    // let async_buy = swap_buy_asset(mango_client.clone());

    // transactions::await_transaction_signature_confirmation(mango_client.clone()).await;

    let connect = ws::try_connect::<RpcSolPubSubClient>(&ws_url).map_err_anyhow()?;
    let client = connect.await.map_err_anyhow()?;

    let foo = client.signature_subscribe(
        "3EtVaf1Go41W1dTkG8PtfrRDrrcBsiXzzWCmtmRr4Ce7YRDuPRJ4mXYhqK7zYsCrVAaCJqsPChCd8yUnPPki4WW1".to_string(),
        Some(RpcSignatureSubscribeConfig { commitment: Some(CommitmentConfig::confirmed()), enable_received_notification: None })
        // meta: Self::Metadata,
        // subscriber: Subscriber<RpcResponse<RpcSignatureResult>>,
        // signature_str: String,
        // config: Option<RpcSignatureSubscribeConfig>,
    );

    // Result<TypedSubscriptionStream<Response<RpcSignatureResult>>, RpcError>

    let sub : TypedSubscriptionStream<Response<RpcSignatureResult>> = foo.unwrap();

    sub.next();

    // async_buy.await;


    Ok(())
}

fn _blockhash_poller() {
    // let recent_confirmed_blockhash = start_blockhash_service(rpc_url.clone()).await;
    // println!("blockhash: {}", recent_confirmed_blockhash.read().unwrap());
}

