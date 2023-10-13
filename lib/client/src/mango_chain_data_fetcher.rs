use anchor_lang::Discriminator;
use anyhow::Context;
use solana_sdk::account::ReadableAccount;
use solana_sdk::pubkey::Pubkey;
use fixed::types::I80F48;
use mango_feeds_connector::account_fetcher_trait::{AccountFetcher, AccountFetcherSync};
use mango_feeds_connector::chain_data_fetcher::ChainDataFetcher;
use mango_v4::state::{Bank, MangoAccount, MangoAccountValue};
use mango_v4::accounts_zerocopy::{KeyedAccountSharedData, LoadZeroCopy};

// use crate::chain_data_fetcher::ChainDataFetcher;

#[async_trait::async_trait]
pub trait MangoChainDataFetcher {

    fn fetch_zero<T: anchor_lang::ZeroCopy + anchor_lang::Owner>(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<T>;

    // this is not mango-related
    async fn fetch_fresh<T: anchor_lang::ZeroCopy + anchor_lang::Owner>(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<T>;

    async fn fetch_fresh_mango_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue>;

    async fn fetch_bank_price(&self, bank: &Pubkey) -> anyhow::Result<I80F48>;

    fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue>;

}



#[async_trait::async_trait]
impl MangoChainDataFetcher for ChainDataFetcher {
    // fetches via RPC, stores in ChainData, returns new version
    async fn fetch_fresh<T: anchor_lang::ZeroCopy + anchor_lang::Owner>(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<T> {
        self.refresh_account_via_rpc(address).await?;
        self.fetch_zero(address)
    }

    // TODO rename
    fn fetch_zero<T: anchor_lang::ZeroCopy + anchor_lang::Owner>(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<T> {
        Ok(*self
            .fetch_raw_account_sync(address)?
            .load::<T>()
            .with_context(|| format!("loading account {}", address))?)
    }

    async fn fetch_fresh_mango_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue> {
        self.refresh_account_via_rpc(address).await?;
        self.fetch_mango_account(address)
    }

    async fn fetch_bank_price(&self, bank: &Pubkey) -> anyhow::Result<I80F48> {
        let bank: Bank = self.fetch_zero(bank)?;
        let oracle = self.fetch_raw_account_sync(&bank.oracle)?;
        let price = bank.oracle_price(&KeyedAccountSharedData::new(bank.oracle, oracle), None)?;
        Ok(price)
    }

    fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue> {
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

}
