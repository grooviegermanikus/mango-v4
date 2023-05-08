use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, info, trace};
use mpsc::unbounded_channel;
use tokio::sync::{mpsc, RwLock};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::time::{interval, sleep};

use services::orderbook_stream_sell::listen_orderbook_feed;

use crate::{mango, services};
use crate::services::asset_price_swap::{BuyPrice, SellPrice};

const STARTUP_DELAY: Duration = Duration::from_secs(2);

struct Coordinator {
    // swap price from router service
    buy_price_stream: UnboundedReceiver<BuyPrice>,
    sell_price_stream: UnboundedReceiver<SellPrice>,
    // orderbook
    last_bid_price_shared: Arc<RwLock<Option<f64>>>,
    last_ask_price_shared: Arc<RwLock<Option<f64>>>,
}

pub async fn run_coordinator_service() {

    let (buy_price_xwrite, mut buy_price_xread) = unbounded_channel();
    let (sell_price_xwrite, mut sell_price_xread) = unbounded_channel();

    let mut coo = Coordinator {
        buy_price_stream: buy_price_xread,
        sell_price_stream: sell_price_xread,
        last_bid_price_shared: Arc::new(RwLock::new(None)),
        last_ask_price_shared: Arc::new(RwLock::new(None)),
    };

    let poll_buy_price = tokio::spawn({
        async move {
            sleep(STARTUP_DELAY).await;
            let mut interval = interval(Duration::from_secs(2));
            loop {
                let price = services::asset_price_swap::call_buy().await;
                debug!("swap buy price: {:?}", price);

                buy_price_xwrite.send(price).unwrap();

                interval.tick().await;
            }
        }
    });

    let poll_sell_price = tokio::spawn({
        async move {
            sleep(STARTUP_DELAY).await;
            let mut interval = interval(Duration::from_secs(2));
            loop {
                let price = services::asset_price_swap::call_sell().await;
                debug!("swap sell price: {:?}", price);

                sell_price_xwrite.send(price).unwrap();

                interval.tick().await;
            }
        }
    });



    let poll_orderbook = tokio::spawn({
        let last_bid_price = coo.last_bid_price_shared.clone();
        let last_ask_price = coo.last_ask_price_shared.clone();
        async move {
            sleep(STARTUP_DELAY).await;
            listen_orderbook_feed(mango::MARKET_ETH_PERP, last_bid_price, last_ask_price).await;
        }
    });

    // buy on jupiter, short on eth-perp
    let main_jup2perp_poller = tokio::spawn({
        let last_bid_price = coo.last_bid_price_shared.clone();
        let last_ask_price = coo.last_ask_price_shared.clone();
        async move {

            let mut interval = interval(Duration::from_secs(2));
            info!("Entering coordinator JUPITER->ETH-PERP loop (interval={:?}) ...", interval.period());
            loop {

                let latest_swap_buy = drain_buy_feed(&mut coo.buy_price_stream);
                debug!("swap latest buy price {:?}", latest_swap_buy);

                let orderbook_bid = last_bid_price.read().await;
                debug!("orderbook(perp) best bid {:?}", *orderbook_bid);

                if let (Some(perp_bid), Some(swap_buy)) = (*orderbook_bid, latest_swap_buy) {
                    let profit = (perp_bid - swap_buy.price) / swap_buy.price;
                    info!("perp-bid {:.2?} vs swap-buy {:.2?}, profit {:.2?}%", perp_bid, swap_buy.price, 100.0 * profit);
                }

                interval.tick().await;
            }
        }
    });

    // buy on jupiter, short on eth-perp
    let main_perp2jup_poller = tokio::spawn({
        let last_bid_price = coo.last_bid_price_shared.clone();
        let last_ask_price = coo.last_ask_price_shared.clone();
        async move {

            let mut interval = interval(Duration::from_secs(2));
            info!("Entering coordinator ETH-PERP->JUPITER loop (interval={:?}) ...", interval.period());
            loop {

                let latest_swap_sell = drain_sell_feed(&mut coo.sell_price_stream);
                debug!("swap latest sell price {:?}", latest_swap_sell);

                let orderbook_ask = last_ask_price.read().await;
                debug!("orderbook(perp) best ask {:?}", *orderbook_ask);

                if let (Some(perp_ask), Some(swap_sell)) = (*orderbook_ask, latest_swap_sell) {
                    let profit = (swap_sell.price - perp_ask) / perp_ask;
                    info!("swap-sell {:.2?} vs perp-ask {:.2?}, profit {:.2?}%", swap_sell.price, perp_ask, 100.0 * profit);
                }

                interval.tick().await;
            }
        }
    });


    main_jup2perp_poller.await.unwrap();
    main_perp2jup_poller.await.unwrap();

}

// drain feeds and get latest value
fn drain_buy_feed(feed: &mut UnboundedReceiver<BuyPrice>) -> Option<BuyPrice> {
    let mut latest = None;
    while let Ok(price) = feed.try_recv() {
        trace!("drain buy price from feed {:?}", price);
        latest = Some(price);
    }
    latest
}

fn drain_sell_feed(feed: &mut UnboundedReceiver<SellPrice>) -> Option<SellPrice> {
    let mut latest = None;
    while let Ok(price) = feed.try_recv() {
        trace!("drain sell price from feed {:?}", price);
        latest = Some(price);
    }
    latest
}

