//! Extension to `account_fetcher.rs` to support Mango v4 accounts
//!
//! This module contains the specific mango types.

use anchor_lang::solana_program::address_lookup_table_account::AddressLookupTableAccount;
use anchor_lang::solana_program::example_mocks::solana_address_lookup_table_program::state::AddressLookupTable;
use anyhow::Context;
use futures::{stream, StreamExt, TryStreamExt};

use solana_sdk::account::ReadableAccount;
use solana_sdk::pubkey::Pubkey;

use mango_v4::state::MangoAccountValue;
use crate::account_fetcher_mangov4::fetch_address_lookup_table;
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

pub async fn account_fetcher_mango_address_lookup_tables(
    fetcher: &dyn AccountFetcher,
    address_lookup_table_pubkeys: Vec<Pubkey>,
) -> anyhow::Result<Vec<AddressLookupTableAccount>> {
    stream::iter(address_lookup_table_pubkeys)
        .then(|k| fetch_address_lookup_table(fetcher, k))
        .try_collect::<Vec<_>>()
        .await
}



