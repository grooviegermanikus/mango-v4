//! Repository to access accounts
//!
//! Delegating to chain_data_fetcher
//! Supports sync calls

use std::sync::{Arc, RwLock};
use std::time::Duration;
use anchor_lang::Discriminator;
use anchor_lang::solana_program::clock::Slot;
use anyhow::Context;
use fixed::types::I80F48;
use mango_feeds_connector::account_fetcher_trait::{AccountFetcher, AccountFetcherSync};
use mango_feeds_connector::chain_data::ChainData;
use mango_feeds_connector::chain_data_fetcher::ChainDataFetcher;
use solana_sdk::account::{AccountSharedData, ReadableAccount};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::Signature;
use mango_v4::accounts_zerocopy::{KeyedAccountSharedData, LoadZeroCopy};
use crate::account_fetchers::{account_fetch_and_map, MangoChainDataFetcher};
use solana_client::nonblocking::rpc_client::RpcClient as RpcClientAsync;
use mango_v4::state::{Bank, MangoAccount, MangoAccountValue};

pub struct MangoAccountRepository {
    pub chain_data_fetcher: Arc<ChainDataFetcher>,
    // pub rpc: &'a RpcClientAsync,
}

impl MangoAccountRepository {
    pub fn new(chain_data: Arc<RwLock<ChainData>>, rpc: RpcClientAsync) -> Self {

        let chain_data_fetcher = Arc::new(ChainDataFetcher {
            chain_data,
            rpc,
        });

        MangoAccountRepository {
            chain_data_fetcher,
            // rpc: &chain_data_fetcher.rpc,
        }
    }

    // note: Generic methods cannot be used in a trait because it is not "object safe"
    // note: cannot be in connector because .load depends on ZeroCopy
    pub fn fetch<T: anchor_lang::ZeroCopy + anchor_lang::Owner>(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<T> {
        Ok(*self.chain_data_fetcher
            .fetch_raw_account_sync(address)?
            .load::<T>()
            .with_context(|| format!("loading account {}", address))?)
    }

}

impl AccountFetcherSync for MangoAccountRepository {
    fn fetch_raw_account_sync(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<AccountSharedData> {
        self.chain_data_fetcher.fetch_raw_account_sync(address)
    }

    fn fetch_program_accounts_sync(&self, program: &Pubkey, discriminator: [u8; 8]) -> anyhow::Result<Vec<(Pubkey, AccountSharedData)>> {
        self.chain_data_fetcher.fetch_program_accounts_sync(program, discriminator)
    }
}

#[async_trait::async_trait]
impl AccountFetcher for MangoAccountRepository {
    async fn fetch_raw_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<AccountSharedData> {
        self.chain_data_fetcher.fetch_raw_account(address).await
    }

    async fn fetch_program_accounts(
        &self,
        program: &Pubkey,
        discriminator: [u8; 8],
    ) -> anyhow::Result<Vec<(Pubkey, AccountSharedData)>> {
        self.chain_data_fetcher.fetch_program_accounts(program, discriminator).await
    }
}

/// utilities
impl MangoAccountRepository {

    /// Return the maximum slot reported for the processing of the signatures
    pub async fn transaction_max_slot(&self, signatures: &[Signature]) -> anyhow::Result<Slot> {
        let statuses = self.chain_data_fetcher.rpc.get_signature_statuses(signatures).await?.value;
        Ok(statuses
            .iter()
            .map(|status_opt| status_opt.as_ref().map(|status| status.slot).unwrap_or(0))
            .max()
            .unwrap_or(0))
    }

    pub async fn refresh_account_via_rpc(&self, address: &Pubkey) -> anyhow::Result<Slot> {
        self.chain_data_fetcher.refresh_account_via_rpc(address).await
    }

    pub async fn refresh_accounts_via_rpc_until_slot(
        &self,
        addresses: &[Pubkey],
        min_slot: Slot,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        self.chain_data_fetcher.refresh_accounts_via_rpc_until_slot(addresses, min_slot, timeout).await
    }


    pub fn fetch_bank_price(&self, bank: &Pubkey) -> anyhow::Result<I80F48> {
        let bank: Bank = account_fetch_and_map(self, bank)?;
        let oracle = self.fetch_raw_account_sync(&bank.oracle)?;
        let price = bank.oracle_price(&KeyedAccountSharedData::new(bank.oracle, oracle), None)?;
        Ok(price)
    }

    pub fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue> {
        let acc = self.fetch_raw_account_sync(address)?;

        let data = acc.data();
        if data.len() < 8 {
            anyhow::bail!(
                "account at {} has only {} bytes of data",
                address,
                data.len()
            );
        }
        let disc_bytes = &data[0..8];
        if disc_bytes != MangoAccount::discriminator() {
            anyhow::bail!("not a mango account at {}", address);
        }

        MangoAccountValue::from_bytes(&data[8..])
            .with_context(|| format!("loading mango account {}", address))
    }

    pub async fn fetch_fresh_mango_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue> {
        self.refresh_account_via_rpc(address).await?;
        self.fetch_mango_account(address)
    }

}

