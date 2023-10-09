mod account_fetcher_donatello;
mod account_fetcher_trait;

use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, trace};
use solana_client::nonblocking::rpc_client::{RpcClient as RpcClientAsync, RpcClient};
use solana_sdk::pubkey::Pubkey;
use mango_v4::state::{MangoAccountValue, PerpMarket};

#[tokio::main]
async fn main() {
    tracing_subscriber_init();

    let rpc_url: String = "https://api.mainnet-beta.solana.com/".to_string();
    // let rpc_url: String = "https://api.devnet.solana.com/".to_string();
    let mango_account_pk: Pubkey = Pubkey::from_str("7v8bovqsYfFfEeiXnGLiGTg2VJAn62hSoSCPidKjKL8w").unwrap();

    // https://app.mango.markets/dashboard
    // PERP-SOL
    let perp_account_pk: Pubkey = Pubkey::from_str("ESdnpnNLgTkBCZRuTJkZLi5wKEZ2z47SG3PJrhundSQ2").unwrap();


    let client = RpcClientAsync::new(rpc_url);

    // load_mango_account(client, mango_account_pk).await;


    load_anchor_account(client, perp_account_pk).await;

}


pub async fn load_mango_account(
    rpc: RpcClient,
    account: Pubkey,
) {
}

pub async fn load_anchor_account(
    rpc: RpcClient,
    account: Pubkey,
) {
}


pub fn tracing_subscriber_init() {
    let format = tracing_subscriber::fmt::format().with_ansi(atty::is(atty::Stream::Stdout));

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .event_format(format)
        .init();
}
