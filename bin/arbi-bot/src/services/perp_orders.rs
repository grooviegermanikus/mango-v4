use std::sync::Arc;
use chrono::Utc;
use solana_sdk::pubkey::Pubkey;
use mango_v4::state::{PerpMarket, PlaceOrderType, Side};
use mango_v4_client::{JupiterSwapMode, MangoClient};
use crate::mango::{MINT_ADDRESS_ETH, MINT_ADDRESS_USDC};
use crate::numerics::{ConversionConf, native_amount, native_amount_to_lot, quote_amount_to_lot};
use std::future::Future;
use std::ops::Deref;
use std::str::FromStr;
use clap::{Args, Parser, Subcommand};
use mango_v4_client::{
    keypair_from_cli, pubkey_from_cli, Client,
    TransactionBuilderConfig,
};

pub async fn buy_asset(mango_client: Arc<MangoClient>) {
    // must be unique
    let client_order_id = Utc::now().timestamp_micros();

    let market_index = mango_client.context.perp_market_indexes_by_name.get("ETH-PERP").unwrap();
    let perp_market = mango_client.context.perp_markets.get(market_index).unwrap().market.clone();

    let order_size_lots = native_amount_to_lot(perp_market.into(), 0.0001);
    println!("order size buy (client id {}): {}", client_order_id, order_size_lots);

    let sig = mango_client.perp_place_order(
        market_index.clone(),
        Side::Bid, 0 /* ignore price */,
        order_size_lots,
        quote_amount_to_lot(perp_market.into(), 100.00),
        client_order_id as u64,
        PlaceOrderType::Market,
        false,
        0,
        64 // max num orders to be skipped based on expiry information in the orderbook
    ).await;

    println!("sig buy: {:?}", sig);
}

// fails ATM due to delegate account
pub async fn sell_asset(mango_client: Arc<MangoClient>) {
    let market_index = mango_client.context.perp_market_indexes_by_name.get("ETH-PERP").unwrap();
    let perp_market = mango_client.context.perp_markets.get(market_index).unwrap().market.clone();

    let order_size_sell = native_amount(perp_market.into(), 0.0001);
    println!("order size sell: {:?}", order_size_sell);
    let sig_sell = mango_client.jupiter_swap(
        Pubkey::from_str(MINT_ADDRESS_ETH).unwrap(),
        Pubkey::from_str(MINT_ADDRESS_USDC).unwrap(),
        order_size_sell,
        10, // TODO 0.1%, 100=1% make configurable
        JupiterSwapMode::ExactIn
    ).await;

    println!("sig sell: {:?}", sig_sell);
}
