mod account_fetcher_mangov4;
mod account_fetcher_trait;

use std::str::FromStr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use mockall::mock;
use tracing::{info, trace};
use account_fetcher_mangov4::{account_fetcher_fetch_mango_account, CachedAccountFetcher, RpcAccountFetcher};
use solana_client::nonblocking::rpc_client::{RpcClient as RpcClientAsync, RpcClient};
use solana_sdk::account::AccountSharedData;
use solana_sdk::pubkey::Pubkey;
use mango_v4::state::{MangoAccountValue, PerpMarket};
use crate::account_fetcher_trait::AccountFetcher;


#[tokio::main]
async fn main() {
    tracing_subscriber_init();

    let rpc_url: String = "https://api.mainnet-beta.solana.com/".to_string();
    // let rpc_url: String = "https://api.devnet.solana.com/".to_string();
    let mango_account_pk: Pubkey = Pubkey::from_str("7v8bovqsYfFfEeiXnGLiGTg2VJAn62hSoSCPidKjKL8w").unwrap();

    // https://app.mango.markets/dashboard
    // PERP-SOL
    let perp_account_pk: Pubkey = Pubkey::from_str("ESdnpnNLgTkBCZRuTJkZLi5wKEZ2z47SG3PJrhundSQ2").unwrap();

    load_mango_account_cached(rpc_url.clone(), mango_account_pk).await;

    load_mango_account(rpc_url.clone(), mango_account_pk).await;

    load_anchor_account(rpc_url.clone(), perp_account_pk).await;

    call_cache_with_mock(mango_account_pk).await;

}

struct MockExampleFetcher {
    pub fetched_mango_calls: AtomicU32,
}

impl MockExampleFetcher {

    pub fn new() -> Self {
        Self {
            fetched_mango_calls: AtomicU32::new(0),
        }
    }

    pub fn assert_call_count(&self, expected: u32) {
        assert_eq!(self.fetched_mango_calls.load(Ordering::SeqCst), expected);
    }

}

#[async_trait::async_trait]
impl AccountFetcher for MockExampleFetcher {
    async fn fetch_raw_account(&self, address: &Pubkey) -> anyhow::Result<AccountSharedData> {
        panic!()
    }

    async fn fetch_raw_account_lookup_table(&self, address: &Pubkey) -> anyhow::Result<AccountSharedData> {
        panic!()
    }

    async fn fetch_program_accounts(&self, program: &Pubkey, discriminator: [u8; 8]) -> anyhow::Result<Vec<(Pubkey, AccountSharedData)>> {
        let call_count = self.fetched_mango_calls.fetch_add(1, Ordering::SeqCst) + 1;
        info!("Call to mocked fetch_program_accounts... {}", call_count);

        Ok(vec![])
    }
}




async fn call_cache_with_mock(account: Pubkey,) {

    let mut mock = Arc::new(MockExampleFetcher::new());

    let mock_fetcher = CachedAccountFetcher::new(mock.clone());
    mock.assert_call_count(0);

    let first_call = mock_fetcher.fetch_program_accounts(&account, [0; 8]).await.unwrap();
    mock.assert_call_count(1);

    let second_call_cached = mock_fetcher.fetch_program_accounts(&account, [0; 8]).await.unwrap();
    mock.assert_call_count(1);

    mock_fetcher.clear_cache();
    let third_call_cached = mock_fetcher.fetch_program_accounts(&account, [0; 8]).await.unwrap();
    mock.assert_call_count(2);
}


pub async fn load_mango_account_cached(
    rpc_url: String,
    account: Pubkey,
) {
    let rpc_client = RpcClientAsync::new(rpc_url);

    let cachedaccount_fetcher = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc: rpc_client,
    })));
    let _mango_account: MangoAccountValue =
        account_fetcher_mangov4::account_fetcher_fetch_mango_account(&*cachedaccount_fetcher, &account).await.unwrap();
    info!("mango account loaded cached");
}


pub async fn load_mango_account(
    rpc_url: String,
    account: Pubkey,
) {
    let rpc_client = RpcClientAsync::new(rpc_url);

    let account_fetcher = Arc::new(RpcAccountFetcher {
        rpc: rpc_client,
    });
    let _mango_account: MangoAccountValue =
        account_fetcher_mangov4::account_fetcher_fetch_mango_account(&*account_fetcher, &account).await.unwrap();
    info!("mango account loaded");
}

pub async fn load_anchor_account(
    rpc_url: String,
    account: Pubkey,
) {
    let rpc_client = RpcClientAsync::new(rpc_url);

    let account_fetcher = Arc::new(CachedAccountFetcher::new(Arc::new(RpcAccountFetcher {
        rpc: rpc_client,
    })));
    let perp_market: PerpMarket =
        account_fetcher_mangov4::account_fetcher_fetch_anchor_account::<PerpMarket>(&*account_fetcher, &account).await.unwrap();
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
