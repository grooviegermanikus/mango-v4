use anchor_lang::Discriminator;
use anyhow::Context;
use solana_sdk::account::ReadableAccount;
use solana_sdk::pubkey::Pubkey;
use mango_v4::state::{MangoAccount, MangoAccountValue};
use crate::chain_data_fetcher::ChainDataFetcher;

#[async_trait::async_trait]
pub trait MangoChainDataFetcher {

    async fn fetch_fresh_mango_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue>;

    fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue>;

}

#[async_trait::async_trait]
impl MangoChainDataFetcher for ChainDataFetcher {

    async fn fetch_fresh_mango_account(
        &self,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue> {
        self.refresh_account_via_rpc(address).await?;
        self.fetch_mango_account(address)
    }

    fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue> {
        let acc = self.fetch_raw(address)?;

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
