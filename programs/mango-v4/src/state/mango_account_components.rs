use anchor_lang::prelude::*;
use checked_math as cm;
use fixed::types::I80F48;
use static_assertions::const_assert_eq;
use std::cmp::Ordering;
use std::mem::size_of;

use crate::state::*;

pub const FREE_ORDER_SLOT: PerpMarketIndex = PerpMarketIndex::MAX;

#[zero_copy]
#[derive(AnchorDeserialize, AnchorSerialize, Debug)]
pub struct TokenPosition {
    // TODO: Why did we have deposits and borrows as two different values
    //       if only one of them was allowed to be != 0 at a time?
    // todo: maybe we want to split collateral and lending?
    // todo: see https://github.com/blockworks-foundation/mango-v4/issues/1
    // todo: how does ftx do this?
    /// The deposit_index (if positive) or borrow_index (if negative) scaled position
    pub indexed_position: I80F48,

    /// index into Group.tokens
    pub token_index: TokenIndex,

    /// incremented when a market requires this position to stay alive
    pub in_use_count: u8,

    pub padding: [u8; 5],

    pub reserved: [u8; 40],
}

unsafe impl bytemuck::Pod for TokenPosition {}
unsafe impl bytemuck::Zeroable for TokenPosition {}

const_assert_eq!(size_of::<TokenPosition>(), 24 + 40);
const_assert_eq!(size_of::<TokenPosition>() % 8, 0);

impl Default for TokenPosition {
    fn default() -> Self {
        TokenPosition {
            indexed_position: I80F48::ZERO,
            token_index: TokenIndex::MAX,
            in_use_count: 0,
            padding: Default::default(),
            reserved: [0; 40],
        }
    }
}

impl TokenPosition {
    pub fn is_active(&self) -> bool {
        self.token_index != TokenIndex::MAX
    }

    pub fn is_active_for_token(&self, token_index: TokenIndex) -> bool {
        self.token_index == token_index
    }

    pub fn native(&self, bank: &Bank) -> I80F48 {
        if self.indexed_position.is_positive() {
            self.indexed_position * bank.deposit_index
        } else {
            self.indexed_position * bank.borrow_index
        }
    }

    pub fn ui(&self, bank: &Bank) -> I80F48 {
        if self.indexed_position.is_positive() {
            (self.indexed_position * bank.deposit_index)
                / I80F48::from_num(10u64.pow(bank.mint_decimals as u32))
        } else {
            (self.indexed_position * bank.borrow_index)
                / I80F48::from_num(10u64.pow(bank.mint_decimals as u32))
        }
    }

    pub fn is_in_use(&self) -> bool {
        self.in_use_count > 0
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct Serum3Orders {
    pub open_orders: Pubkey,

    // tracks reserved funds in open orders account,
    // used for bookkeeping of potentital loans which
    // can be charged with loan origination fees
    // e.g. serum3 settle funds ix
    pub previous_native_coin_reserved: u64,
    pub previous_native_pc_reserved: u64,

    pub market_index: Serum3MarketIndex,

    /// Store the base/quote token index, so health computations don't need
    /// to get passed the static SerumMarket to find which tokens a market
    /// uses and look up the correct oracles.
    pub base_token_index: TokenIndex,
    pub quote_token_index: TokenIndex,

    pub padding: [u8; 2],

    pub reserved: [u8; 64],
}
const_assert_eq!(size_of::<Serum3Orders>(), 32 + 8 * 2 + 2 * 3 + 2 + 64);
const_assert_eq!(size_of::<Serum3Orders>() % 8, 0);

unsafe impl bytemuck::Pod for Serum3Orders {}
unsafe impl bytemuck::Zeroable for Serum3Orders {}

impl Serum3Orders {
    pub fn is_active(&self) -> bool {
        self.market_index != Serum3MarketIndex::MAX
    }

    pub fn is_active_for_market(&self, market_index: Serum3MarketIndex) -> bool {
        self.market_index == market_index
    }
}

impl Default for Serum3Orders {
    fn default() -> Self {
        Self {
            open_orders: Pubkey::default(),
            market_index: Serum3MarketIndex::MAX,
            base_token_index: TokenIndex::MAX,
            quote_token_index: TokenIndex::MAX,
            reserved: [0; 64],
            padding: Default::default(),
            previous_native_coin_reserved: 0,
            previous_native_pc_reserved: 0,
        }
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct PerpPositions {
    pub market_index: PerpMarketIndex,
    pub padding: [u8; 6],

    /// Active position size, measured in base lots
    pub base_position_lots: i64,
    /// Active position in quote (conversation rate is that of the time the order was settled)
    /// measured in native quote
    pub quote_position_native: I80F48,

    /// Tracks what the position is to calculate average entry  & break even price
    pub base_entry_lots: i64,
    pub quote_entry_native: i64,
    pub quote_exit_native: i64,

    /// Already settled funding
    pub long_settled_funding: I80F48,
    pub short_settled_funding: I80F48,

    /// Base lots in bids
    pub bids_base_lots: i64,
    /// Base lots in asks
    pub asks_base_lots: i64,

    /// Liquidity mining rewards
    // pub mngo_accrued: u64,

    /// Amount that's on EventQueue waiting to be processed
    pub taker_base_lots: i64,
    pub taker_quote_lots: i64,

    pub reserved: [u8; 64],
}

impl std::fmt::Debug for PerpPositions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PerpAccount")
            .field("market_index", &self.market_index)
            .field("base_position_lots", &self.base_position_lots)
            .field("quote_position_native", &self.quote_position_native)
            .field("bids_base_lots", &self.bids_base_lots)
            .field("asks_base_lots", &self.asks_base_lots)
            .field("taker_base_lots", &self.taker_base_lots)
            .field("taker_quote_lots", &self.taker_quote_lots)
            .finish()
    }
}
const_assert_eq!(size_of::<PerpPositions>(), 8 + 8 * 8 + 3 * 16 + 64);
const_assert_eq!(size_of::<PerpPositions>() % 8, 0);

unsafe impl bytemuck::Pod for PerpPositions {}
unsafe impl bytemuck::Zeroable for PerpPositions {}

impl Default for PerpPositions {
    fn default() -> Self {
        Self {
            market_index: PerpMarketIndex::MAX,
            base_position_lots: 0,
            quote_position_native: I80F48::ZERO,
            base_entry_lots: 0,
            quote_entry_native: 0,
            quote_exit_native: 0,
            bids_base_lots: 0,
            asks_base_lots: 0,
            taker_base_lots: 0,
            taker_quote_lots: 0,
            reserved: [0; 64],
            long_settled_funding: I80F48::ZERO,
            short_settled_funding: I80F48::ZERO,
            padding: Default::default(),
        }
    }
}

impl PerpPositions {
    /// Add taker trade after it has been matched but before it has been process on EventQueue
    pub fn add_taker_trade(&mut self, side: Side, base_lots: i64, quote_lots: i64) {
        match side {
            Side::Bid => {
                self.taker_base_lots = cm!(self.taker_base_lots + base_lots);
                self.taker_quote_lots = cm!(self.taker_quote_lots - quote_lots);
            }
            Side::Ask => {
                self.taker_base_lots = cm!(self.taker_base_lots - base_lots);
                self.taker_quote_lots = cm!(self.taker_quote_lots + quote_lots);
            }
        }
    }
    /// Remove taker trade after it has been processed on EventQueue
    pub fn remove_taker_trade(&mut self, base_change: i64, quote_change: i64) {
        self.taker_base_lots = cm!(self.taker_base_lots - base_change);
        self.taker_quote_lots = cm!(self.taker_quote_lots - quote_change);
    }

    pub fn is_active(&self) -> bool {
        self.market_index != PerpMarketIndex::MAX
    }

    pub fn is_active_for_market(&self, market_index: PerpMarketIndex) -> bool {
        self.market_index == market_index
    }

    /// This assumes settle_funding was already called
    pub fn change_base_position(&mut self, perp_market: &mut PerpMarket, base_change: i64) {
        let start = self.base_position_lots;
        self.base_position_lots += base_change;
        perp_market.open_interest += self.base_position_lots.abs() - start.abs();
    }

    /// Move unrealized funding payments into the quote_position
    pub fn settle_funding(&mut self, perp_market: &PerpMarket) {
        match self.base_position_lots.cmp(&0) {
            Ordering::Greater => {
                self.quote_position_native -= (perp_market.long_funding
                    - self.long_settled_funding)
                    * I80F48::from_num(self.base_position_lots);
            }
            Ordering::Less => {
                self.quote_position_native -= (perp_market.short_funding
                    - self.short_settled_funding)
                    * I80F48::from_num(self.base_position_lots);
            }
            Ordering::Equal => (),
        }
        self.long_settled_funding = perp_market.long_funding;
        self.short_settled_funding = perp_market.short_funding;
    }

    /// Update the quote entry position
    pub fn change_quote_entry(&mut self, base_change: i64, quote_change: i64) {
        if base_change == 0 {
            return;
        }
        let old_position = self.base_position_lots;
        let is_increasing = old_position == 0 || old_position.signum() == base_change.signum();
        match is_increasing {
            true => {
                self.quote_entry_native = cm!(self.quote_entry_native + quote_change);
                self.base_entry_lots = cm!(self.base_entry_lots + base_change);
            }
            false => {
                let new_position = cm!(old_position + base_change);
                self.quote_exit_native = cm!(self.quote_exit_native + quote_change);
                let is_overflow = old_position.signum() == -new_position.signum();
                if new_position == 0 {
                    self.quote_entry_native = 0;
                    self.quote_exit_native = 0;
                    self.base_entry_lots = 0;
                }
                if is_overflow {
                    self.quote_entry_native = cm!(((new_position as f64) * (quote_change as f64)
                        / (base_change as f64))
                        .round()) as i64;
                    self.quote_exit_native = 0;
                    self.base_entry_lots = new_position;
                }
            }
        }
    }

    /// Change the base and quote positions as the result of a trade
    pub fn change_base_and_entry_positions(
        &mut self,
        perp_market: &mut PerpMarket,
        base_change: i64,
        quote_change: i64,
    ) {
        self.change_quote_entry(base_change, quote_change);
        self.change_base_position(perp_market, base_change);
    }

    /// Calculate the average entry price of the position
    pub fn get_avg_entry_price(&self) -> I80F48 {
        if self.base_entry_lots == 0 {
            return I80F48::ZERO; // TODO: What should this actually return? Error? NaN?
        }
        (I80F48::from(self.quote_entry_native) / I80F48::from(self.base_entry_lots)).abs()
    }

    /// Calculate the break even price of the position
    pub fn get_break_even_price(&self) -> I80F48 {
        if self.base_position_lots == 0 {
            return I80F48::ZERO; // TODO: What should this actually return? Error? NaN?
        }
        (I80F48::from(self.quote_entry_native + self.quote_exit_native)
            / I80F48::from(self.base_position_lots))
        .abs()
    }
}

#[zero_copy]
#[derive(AnchorSerialize, AnchorDeserialize, Debug)]
pub struct PerpOpenOrders {
    pub order_side: Side, // TODO: storing enums isn't POD
    pub padding1: [u8; 1],
    pub order_market: PerpMarketIndex,
    pub padding2: [u8; 4],
    pub client_order_id: u64,
    pub order_id: i128,
    pub reserved: [u8; 64],
}

impl Default for PerpOpenOrders {
    fn default() -> Self {
        Self {
            order_side: Side::Bid,
            padding1: Default::default(),
            order_market: FREE_ORDER_SLOT,
            padding2: Default::default(),
            client_order_id: 0,
            order_id: 0,
            reserved: [0; 64],
        }
    }
}

unsafe impl bytemuck::Pod for PerpOpenOrders {}
unsafe impl bytemuck::Zeroable for PerpOpenOrders {}

const_assert_eq!(size_of::<PerpOpenOrders>(), 1 + 1 + 2 + 4 + 8 + 16 + 64);
const_assert_eq!(size_of::<PerpOpenOrders>() % 8, 0);

#[macro_export]
macro_rules! account_seeds {
    ( $account:expr ) => {
        &[
            $account.group.as_ref(),
            b"MangoAccount".as_ref(),
            $account.owner.as_ref(),
            &$account.account_num.to_le_bytes(),
            &[$account.bump],
        ]
    };
}

pub use account_seeds;

#[cfg(test)]
mod tests {
    use crate::state::{OracleConfig, PerpMarket};
    use anchor_lang::prelude::Pubkey;
    use fixed::types::I80F48;
    use rand::Rng;

    use super::PerpPositions;

    fn create_perp_position(base_pos: i64, quote_pos: i64, entry_pos: i64) -> PerpPositions {
        let mut pos = PerpPositions::default();
        pos.base_position_lots = base_pos;
        pos.quote_position_native = I80F48::from(quote_pos);
        pos.quote_entry_native = entry_pos;
        pos.quote_exit_native = 0;
        pos.base_entry_lots = base_pos;
        pos
    }

    fn create_perp_market() -> PerpMarket {
        return PerpMarket {
            group: Pubkey::new_unique(),
            base_token_index: 0,
            perp_market_index: 0,
            name: Default::default(),
            oracle: Pubkey::new_unique(),
            oracle_config: OracleConfig {
                conf_filter: I80F48::ZERO,
            },
            bids: Pubkey::new_unique(),
            asks: Pubkey::new_unique(),
            event_queue: Pubkey::new_unique(),
            quote_lot_size: 1,
            base_lot_size: 1,
            maint_asset_weight: I80F48::from(1),
            init_asset_weight: I80F48::from(1),
            maint_liab_weight: I80F48::from(1),
            init_liab_weight: I80F48::from(1),
            liquidation_fee: I80F48::ZERO,
            maker_fee: I80F48::ZERO,
            taker_fee: I80F48::ZERO,
            min_funding: I80F48::ZERO,
            max_funding: I80F48::ZERO,
            impact_quantity: 0,
            long_funding: I80F48::ZERO,
            short_funding: I80F48::ZERO,
            funding_last_updated: 0,
            open_interest: 0,
            seq_num: 0,
            fees_accrued: I80F48::ZERO,
            bump: 0,
            base_token_decimals: 0,
            reserved: [0; 128],
            padding1: Default::default(),
            padding2: Default::default(),
            registration_time: 0,
        };
    }

    #[test]
    fn test_quote_entry_long_increasing_from_zero() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(0, 0, 0);
        // Go long 10 @ 10
        pos.change_base_and_entry_positions(&mut market, 10, -100);
        assert_eq!(pos.quote_entry_native, -100);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(10));
    }

    #[test]
    fn test_quote_entry_short_increasing_from_zero() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(0, 0, 0);
        // Go short 10 @ 10
        pos.change_base_and_entry_positions(&mut market, -10, 100);
        assert_eq!(pos.quote_entry_native, 100);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(10));
    }

    #[test]
    fn test_quote_entry_long_increasing_from_long() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(10, -100, -100);
        // Go long 10 @ 30
        pos.change_base_and_entry_positions(&mut market, 10, -300);
        assert_eq!(pos.quote_entry_native, -400);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(20));
    }

    #[test]
    fn test_quote_entry_short_increasing_from_short() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(-10, 100, 100);
        // Go short 10 @ 10
        pos.change_base_and_entry_positions(&mut market, -10, 300);
        assert_eq!(pos.quote_entry_native, 400);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(20));
    }

    #[test]
    fn test_quote_entry_long_decreasing_from_short() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(-10, 100, 100);
        // Go long 5 @ 50
        pos.change_base_and_entry_positions(&mut market, 5, 250);
        assert_eq!(pos.quote_entry_native, 100);
        assert_eq!(pos.base_entry_lots, -10);
        assert_eq!(pos.quote_exit_native, 250);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(10)); // Entry price remains the same when decreasing
    }

    #[test]
    fn test_quote_entry_short_decreasing_from_long() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(10, -100, -100);
        // Go short 5 @ 50
        pos.change_base_and_entry_positions(&mut market, -5, -250);
        assert_eq!(pos.quote_entry_native, -100);
        assert_eq!(pos.base_entry_lots, 10);
        assert_eq!(pos.quote_exit_native, -250);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(10)); // Entry price remains the same when decreasing
    }

    #[test]
    fn test_quote_entry_long_close_with_short() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(10, -100, -100);
        // Go short 10 @ 50
        pos.change_base_and_entry_positions(&mut market, -10, 250);
        assert_eq!(pos.quote_entry_native, 0);
        assert_eq!(pos.quote_exit_native, 0);
        assert_eq!(pos.base_entry_lots, 0);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(0)); // Entry price zero when no position
    }

    #[test]
    fn test_quote_entry_short_close_with_long() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(-10, 100, 100);
        // Go long 10 @ 50
        pos.change_base_and_entry_positions(&mut market, 10, -250);
        assert_eq!(pos.quote_entry_native, 0);
        assert_eq!(pos.quote_exit_native, 0);
        assert_eq!(pos.base_entry_lots, 0);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(0)); // Entry price zero when no position
    }

    #[test]
    fn test_quote_entry_long_close_short_with_overflow() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(10, -100, -100);
        // Go short 15 @ 20
        pos.change_base_and_entry_positions(&mut market, -15, 300);
        assert_eq!(pos.quote_entry_native, 100);
        assert_eq!(pos.quote_exit_native, 0);
        assert_eq!(pos.base_entry_lots, -5);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(20)); // Entry price zero when no position
    }

    #[test]
    fn test_quote_entry_short_close_long_with_overflow() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(-10, 100, 100);
        // Go short 15 @ 20
        pos.change_base_and_entry_positions(&mut market, 15, -300);
        assert_eq!(pos.quote_entry_native, -100);
        assert_eq!(pos.quote_exit_native, 0);
        assert_eq!(pos.base_entry_lots, 5);
        assert_eq!(pos.get_avg_entry_price(), I80F48::from(20)); // Entry price zero when no position
    }

    #[test]
    fn test_quote_entry_break_even_price() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(0, 0, 0);
        // Buy 11 @ 10,000
        pos.change_base_and_entry_positions(&mut market, 11, -11 * 10_000);
        // Sell 1 @ 12,000
        pos.change_base_and_entry_positions(&mut market, -1, 12_000);
        assert_eq!(pos.quote_entry_native, -11 * 10_000);
        assert_eq!(pos.quote_exit_native, 12_000);
        assert_eq!(pos.base_entry_lots, 11);
        assert_eq!(pos.base_position_lots, 10);
        assert_eq!(pos.get_break_even_price(), I80F48::from(9_800)); // We made 2k on the trade, so we can sell our contract up to a loss of 200 each
    }

    #[test]
    fn test_quote_entry_multiple_and_reversed_changes_return_entry_to_zero() {
        let mut market = create_perp_market();
        let mut pos = create_perp_position(0, 0, 0);

        // Generate array of random trades
        let mut rng = rand::thread_rng();
        let mut trades: Vec<[i64; 2]> = Vec::with_capacity(500);
        for _ in 0..trades.capacity() {
            let qty: i64 = rng.gen_range(-1000..=1000);
            let px: f64 = rng.gen_range(0.1..=100.0);
            let quote: i64 = (-qty as f64 * px).round() as i64;
            trades.push([qty, quote]);
        }
        // Apply all of the trades going forward
        trades.iter().for_each(|[qty, quote]| {
            pos.change_base_and_entry_positions(&mut market, *qty, *quote);
        });
        // base_position should be sum of all base quantities
        assert_eq!(
            pos.base_position_lots,
            trades.iter().map(|[qty, _]| qty).sum::<i64>()
        );
        // Reverse out all the trades
        trades.iter().for_each(|[qty, quote]| {
            pos.change_base_and_entry_positions(&mut market, -*qty, -*quote);
        });
        // base position should be 0
        assert_eq!(pos.base_position_lots, 0);
        // quote entry position should be 0
        assert_eq!(pos.quote_entry_native, 0);
        // quote exit should be 0
        assert_eq!(pos.quote_exit_native, 0);
        // base entry lots should be 0
        assert_eq!(pos.base_entry_lots, 0);
    }
}