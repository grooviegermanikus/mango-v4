// Extension to `account_fetchers` to support Mango v4 accounts
//
// This module contains the specific mango types.

use anchor_lang::solana_program::address_lookup_table_account::AddressLookupTableAccount;
use anyhow::Context;
use futures::{stream, StreamExt, TryStreamExt};
use mango_feeds_connector::account_fetcher::fetch_address_lookup_table;
use mango_feeds_connector::account_fetcher_trait::{AccountFetcher, AccountFetcherSync};

use solana_sdk::account::ReadableAccount;
use solana_sdk::pubkey::Pubkey;

use mango_v4::state::MangoAccountValue;
use crate::account_fetchers::AccountFetcherPlus;


// // Can't be in the trait, since then it would no longer be object-safe...
// pub async fn account_fetcher_fetch_mango_account(
//     fetcher: &dyn AccountFetcher,
//     address: &Pubkey,
// ) -> anyhow::Result<MangoAccountValue> {
//     let account = fetcher.fetch_raw_account(address).await?;
//     let data: &[u8] = &account.data();
//     MangoAccountValue::from_bytes(&data[8..])
//         .with_context(|| format!("deserializing mango account {}", address))
// }

pub fn account_fetcher_sync_fetch_mango_account(
    fetcher: &dyn AccountFetcherSync,
    address: &Pubkey,
) -> anyhow::Result<MangoAccountValue> {
    let account = fetcher.fetch_raw_account_sync(address)?;
    let data: &[u8] = &account.data();
    MangoAccountValue::from_bytes(&data[8..])
        .with_context(|| format!("deserializing mango account {}", address))
}

// Batch-lookup for ALTs
//
// # Examples for use with mango-v4-client
// ```
// let context: MangoGroupContext = ...;
//
// account_fetcher_mango_address_lookup_tables(
//    &account_fetcher, context.address_lookup_table).await
// ```
// pub async fn account_fetcher_mango_address_lookup_tables(
//     fetcher: &dyn AccountFetcher,
//     address_lookup_table_pubkeys: Vec<Pubkey>,
// ) -> anyhow::Result<Vec<AddressLookupTableAccount>> {
//     stream::iter(address_lookup_table_pubkeys)
//         .then(|k| fetch_address_lookup_table(fetcher, k))
//         .try_collect::<Vec<_>>()
//         .await
// }



