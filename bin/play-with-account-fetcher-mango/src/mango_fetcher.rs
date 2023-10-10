use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

use async_once_cell::unpin::Lazy;

use anyhow::Context;

use anchor_client::ClientError;
use anchor_lang::AccountDeserialize;

use solana_client::nonblocking::rpc_client::RpcClient as RpcClientAsync;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::pubkey::Pubkey;

use mango_v4::state::MangoAccountValue;
use crate::account_fetcher_trait::AccountFetcher;



// Can't be in the trait, since then it would no longer be object-safe...
pub async fn account_fetcher_fetch_mango_account(
    fetcher: &dyn AccountFetcher,
    address: &Pubkey,
) -> anyhow::Result<MangoAccountValue> {
    let account = fetcher.fetch_raw_account(address).await?;
    let data: &[u8] = &account.data();
    MangoAccountValue::from_bytes(&data[8..])
        .with_context(|| format!("deserializing mango account {}", address))
}


