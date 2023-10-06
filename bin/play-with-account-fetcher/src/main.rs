mod account_fetcher_donatello;

use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, trace};
use crate::account_fetcher_donatello::{account_fetcher_fetch_mango_account, CachedAccountFetcher, RpcAccountFetcher};
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
    let account_fetcher = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc,
    })));
    let mango_account: MangoAccountValue =
        account_fetcher_donatello::account_fetcher_fetch_mango_account(&*account_fetcher, &account).await.unwrap();
    // info!("mango account: {:?}", mango_account);
    info!("mango account loaded");
}

pub async fn load_anchor_account(
    rpc: RpcClient,
    account: Pubkey,
) {
    let account_fetcher = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc,
    })));
    let perp_market: PerpMarket =
        account_fetcher_donatello::account_fetcher_fetch_anchor_account::<PerpMarket>(&*account_fetcher, &account).await.unwrap();
    info!("perp account loaded: base_decimals={}", perp_market.base_decimals);
}

fn instances(rpc1: RpcClientAsync, rpc2: RpcClientAsync, rpc3: RpcClientAsync) {

    let account_fetcher = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc: rpc1,
    })));

    let _ = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc: rpc2,
    })));

    let _ = RpcAccountFetcher {
        rpc: rpc3,
    };


}

pub fn tracing_subscriber_init() {
    let format = tracing_subscriber::fmt::format().with_ansi(atty::is(atty::Stream::Stdout));

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .event_format(format)
        .init();
}