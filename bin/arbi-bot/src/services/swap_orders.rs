use std::sync::Arc;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use mango_v4_client::{JupiterSwapMode, MangoClient};
use crate::mango::{MINT_ADDRESS_ETH, MINT_ADDRESS_USDC};
use crate::numerics::{native_amount, native_amount2, native_amount_to_lot, quote_amount_to_lot};
use std::future::Future;
use std::ops::Deref;
use std::str::FromStr;
use anyhow::anyhow;
use log::debug;
use serde::{Deserialize, Serialize};
use solana_sdk::signature::Signature;

// bps
const SLIPPAGE: u64 = 5;

pub async fn swap_sell_asset(mango_client: Arc<MangoClient>) -> anyhow::Result<Signature> {
    let market_index = mango_client.context.token_indexes_by_name.get("ETH (Portal)").unwrap();
    let market = mango_client.context.tokens.get(market_index).unwrap();

    let order_size_sell = native_amount2(market.decimals as u32, 0.001);

    debug!("swap order sell with size {:?}", order_size_sell);
    let sig_sell = mango_client.jupiter_swap(
        Pubkey::from_str(MINT_ADDRESS_ETH).unwrap(),
        Pubkey::from_str(MINT_ADDRESS_USDC).unwrap(),
        order_size_sell,
        SLIPPAGE, // TODO 0.01%, 100=1% make configurable
        JupiterSwapMode::ExactIn
    ).await;

    debug!("tx-sig swap sell: {:?}", sig_sell);

    sig_sell
}

// only return sig, caller must check for progress/confirmation
pub async fn swap_buy_asset(mango_client: Arc<MangoClient>) -> anyhow::Result<Signature> {
    let market_index = mango_client.context.token_indexes_by_name.get("ETH (Portal)").unwrap();
    let market = mango_client.context.tokens.get(market_index).unwrap();

    let order_size_buy = native_amount2(market.decimals as u32, 0.001);

    debug!("swap order buy with size {:?}", order_size_buy);
    let sig_buy = mango_client.jupiter_swap(
        Pubkey::from_str(MINT_ADDRESS_USDC).unwrap(),
        Pubkey::from_str(MINT_ADDRESS_ETH).unwrap(),
        order_size_buy,
        SLIPPAGE, // TODO 0.1%, 100=1% make configurable
        JupiterSwapMode::ExactOut
    ).await;

    debug!("tx-sig swap buy: {:?}", sig_buy);
    // TODO return sig

    // Error Message: Slippage tolerance exceeded
    sig_buy
}

