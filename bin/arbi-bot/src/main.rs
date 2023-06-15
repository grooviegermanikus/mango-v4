mod mango;
mod services;
mod coordinator;
mod numerics;

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
use solana_sdk::signature::{Keypair, Signature};
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

use solana_client::rpc_response::SlotUpdate;
use jsonrpc_core::futures::StreamExt;
use log::info;
use solana_client::nonblocking::pubsub_client::PubsubClient;
use solana_client::nonblocking::rpc_client::RpcClient;
// use jsonrpc_core_client::transports::ws;
// use jsonrpc_core_client::TypedSubscriptionStream;

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


// command args for testnet see /Users/stefan/mango/notes/BOT1
#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    env_logger::init_from_env(
        env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV,
                 "info,arbi_bot=trace"),
    );


    let cli = Cli::parse_from(std::env::args_os());

    let rpc_url = cli.rpc_url;
    let ws_url = rpc_url.replace("https", "wss").replace("http", "ws");

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
                Some(Duration::from_secs(12)),
                TransactionBuilderConfig {
                    prioritization_micro_lamports: Some(1),
                },
            ),
            cli.mango_account,
            owner.clone(),
        ).await?);


    let coordinator_thread = tokio::spawn(coordinator::run_coordinator_service(mango_client.clone()));
    coordinator_thread.await?;

    // play with confirmation
    // let async_buy = swap_buy_asset(mango_client.clone());

    // transactions::await_transaction_signature_confirmation(mango_client.clone()).await;

    // let rpc_client = RpcClient::new(rpc_url.clone());
    // play_with_sigaturesubscription(&ws_url, mango_client, rpc_client).await;


    // pub slot: Slot,
    // pub confirmations: Option<usize>,  // None = rooted
    // pub status: TransactionResult<()>, // legacy field
    // pub err: Option<TransactionError>,
    // pub confirmation_status: Option<TransactionConfirmationStatus>,


    // let connect = ws::try_connect::<RpcSolPubSubClient>(&ws_url).map_err_anyhow()?;
    // let client = connect.await.map_err_anyhow()?;

    // Signature::from_str()

    // let foo = client.signature_subscribe(
    //     "3EtVaf1Go41W1dTkG8PtfrRDrrcBsiXzzWCmtmRr4Ce7YRDuPRJ4mXYhqK7zYsCrVAaCJqsPChCd8yUnPPki4WW1".to_string(),
    //     Some(RpcSignatureSubscribeConfig { commitment: Some(CommitmentConfig::confirmed()), enable_received_notification: None })
    //     // meta: Self::Metadata,
    //     // subscriber: Subscriber<RpcResponse<RpcSignatureResult>>,
    //     // signature_str: String,
    //     // config: Option<RpcSignatureSubscribeConfig>,
    // );

    // Result<TypedSubscriptionStream<Response<RpcSignatureResult>>, RpcError>

    // let mut sub : TypedSubscriptionStream<Response<RpcSignatureResult>> = foo.map_err_anyhow().unwrap();
    // let mut slot_sub: TypedSubscriptionStream<Arc<SlotUpdate>> = client.slots_updates_subscribe().map_err_anyhow()?;

    // slot_sub.next();

    // sub.next().await;

    // async_buy.await;


    Ok(())
}

async fn play_with_sigaturesubscription(ws_url: &String, mango_client: Arc<MangoClient>, rpc_client: RpcClient) -> Result<(), anyhow::Error> {
    println!("Connected to {}", ws_url);
    let client = PubsubClient::new(&ws_url).await?;

    let async_buy = swap_buy_asset(mango_client.clone());
    let sig = async_buy.await;
    let sigs = &[sig];

    let subscribe_handle = tokio::spawn(async move {
        let sig = sig.clone();
        let (mut stream, fn_unsusbscribe) = client.signature_subscribe(
            &sig, Some(RpcSignatureSubscribeConfig { commitment: None, enable_received_notification: Some(true) })).await.unwrap();

        while let Some(msg) = stream.next().await {
            match msg.value {
                RpcSignatureResult::ProcessedSignature(s) => {
                    log::info!("processed err={:?}", s);
                    // break;
                    break;
                }
                RpcSignatureResult::ReceivedSignature(_) => {
                    log::info!("received");
                }
            }
        }

        fn_unsusbscribe();
    });


    let asdf = rpc_client.get_signature_statuses(sigs);

    let sdfs = asdf.await.unwrap().value[0].clone();
    let tx_status = sdfs.unwrap();
    info!("tx_status={:?}", tx_status);

    subscribe_handle.await;

    Ok(())
}

fn _blockhash_poller() {
    // let recent_confirmed_blockhash = start_blockhash_service(rpc_url.clone()).await;
    // println!("blockhash: {}", recent_confirmed_blockhash.read().unwrap());
}

