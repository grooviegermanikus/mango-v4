
/// Suggest to import the module like this:
/// ```
/// use mango_v4_client::account_fetchers;
/// ```
///
/// And use the types with the module qualifier:
/// ```
/// account_fetcher: &dyn account_fetchers::AccountFetcherSync,
/// ```
///

pub use mango_feeds_connector::account_fetcher::*;
pub use mango_feeds_connector::chain_data::*;
pub use mango_feeds_connector::chain_data_fetcher::*;
pub use crate::mango_chain_data_fetcher::*;
pub use crate::mango_account_fetcher::*;
pub use mango_feeds_connector::account_fetcher_trait::*;
use solana_sdk::pubkey::Pubkey;
use mango_v4::state::MangoAccountValue;

#[async_trait::async_trait]
pub trait MangoAccountFetcher {

    // Can't be in the trait, since then it would no longer be object-safe...
    async fn account_fetcher_fetch_mango_account(
        fetcher: &dyn AccountFetcher,
        address: &Pubkey,
    ) -> anyhow::Result<MangoAccountValue>;

}

// TODO do we need both?
pub trait AccountFetcherPlus: MangoAccountFetcher + AccountFetcherSync + AccountFetcher {

    fn fetch_mango_account(&self, address: &Pubkey) -> anyhow::Result<MangoAccountValue>;

}

