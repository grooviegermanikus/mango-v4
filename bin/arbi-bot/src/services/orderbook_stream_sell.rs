use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use log::{debug, error, info, trace, warn};
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use serde_json::{from_str, json, Value};
use tokio::sync::RwLock;
use tokio_tungstenite::tungstenite;
use tokio_tungstenite::tungstenite::{connect, Message};
use tokio_tungstenite::tungstenite::client::connect_with_config;
use url::Url;

#[derive(Debug, Copy, Clone)]
pub struct SellPrice {
    // ETH in USDC - 1901,59495311
    pub price: f64,
    pub quantity: f64,
    pub approx_timestamp: Instant,
}

// mango-feeds
type OrderbookLevel = [f64; 2];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum OrderbookSide {
    Bid = 0,
    Ask = 1,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct OrderbookUpdate {
    pub market: String,
    pub side: OrderbookSide,
    pub update: Vec<OrderbookLevel>,
    pub slot: u64,
    pub write_version: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct OrderbookCheckpoint {
    pub market: String,
    pub bids: Vec<OrderbookLevel>,
    pub asks: Vec<OrderbookLevel>,
    pub slot: u64,
    pub write_version: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WsSubscription {
    pub command: String,
    pub market_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct Subscriptions {
    pub market_id: String,
}

#[derive(Default)]
struct Orderbook {
    pub bids: BTreeMap<OrderedFloat<f64>, f64>,
    pub asks: BTreeMap<OrderedFloat<f64>, f64>,
}

impl Orderbook {

    fn update_bid_price(&mut self, price: f64, quantity: f64) {
        assert!(quantity.is_sign_positive(), "bid quantity must be non-negative but was <{}>", price);
        let price = OrderedFloat(price);
        if quantity != 0.0 {
            if !price.is_sign_positive() {
                // TODO check: orderbook asks [(-9.223372036854774e16, 0.1), (1946.14, 0.0235), (1947.5, 0.2353), ...
                warn!("bid price must be non-negative but was <{}>", price);
                return;
            }
            self.bids.insert(price, quantity);
        } else {
            self.bids.remove(&price);
        }
    }

    fn get_highest_bid_price(&self) -> Option<f64> {
        self.bids.last_key_value().map(|(k, _)| k.0)
    }

    fn update_ask_price(&mut self, price: f64, quantity: f64) {
        assert!(quantity.is_sign_positive(), "ask quantity must be non-negative but was <{}>", price);
        let price = OrderedFloat(price);
        if quantity != 0.0 {
            if !price.is_sign_positive() {
                warn!("ask price must be non-negative but was <{}>", price);
                return;
            }
            self.asks.insert(price, quantity);
        } else {
            self.asks.remove(&price);
        }
    }

    fn get_lowest_ask_price(&self) -> Option<f64> {
        self.bids.first_key_value().map(|(k, _)| k.0)
    }

    fn dump(&self) {
        info!("orderbook bids {:?}", self.bids.iter().map(|(k, v)| (k.0, v)).collect::<Vec<_>>());
        info!("orderbook asks {:?}", self.asks.iter().map(|(k, v)| (k.0, v)).collect::<Vec<_>>());
    }
}

// requires running "service-mango-orderbook" - see README
pub async fn listen_orderbook_feed(market_id: &str,
                                   highest_bid_price: Arc<RwLock<Option<f64>>>,
                                   lowest_ask_price: Arc<RwLock<Option<f64>>>) {

    let (mut socket, response) =
        connect(Url::parse("wss://api.mngo.cloud/orderbook/v1/").unwrap()).expect("Can't connect");

    if response.status() != 101 {
        // TODO implement reconnects
        panic!("Error connecting to the server: {:?}", response);
    }
    // Response { status: 101, version: HTTP/1.1, headers: {"connection": "Upgrade", "upgrade": "websocket", "sec-websocket-accept": "ppgfXDDxtQBmL0eczLMf5VGbFIo="}, body: () }

    // subscriptions= {"command":"subscribe","marketId":"ESdnpnNLgTkBCZRuTJkZLi5wKEZ2z47SG3PJrhundSQ2"}
    let sub = &WsSubscription {
        command: "subscribe".to_string(),
        market_id: market_id.to_string(),
    };
    // Ok(Text("{\"success\":false,\"message\":\"market not found\"}"))
    // Ok(Text("{\"success\":true,\"message\":\"subscribed\"}"))

    socket.write_message(Message::text(json!(sub).to_string())).unwrap();

    let mut orderbook: Orderbook = Orderbook::default();

    loop {
        match socket.read_message() {
            Ok(msg) => {
                trace!("Received: {}", msg);
            }
            Err(e) => {
                match e {
                    tungstenite::Error::ConnectionClosed => {
                        error!("Connection closed");
                        break;
                    }
                    _ => {}
                }
                error!("Error reading message: {:?}", e);
                break;
            }
        }
        let msg = socket.read_message().unwrap();

        let msg = match msg {
            tungstenite::Message::Text(s) => { s }
            _ => continue
        };

        let plain = from_str::<Value>(&msg).expect("Can't parse to JSON");

        // detect checkpoint messages via property bid+ask
        let is_checkpoint_message = plain.get("bids").is_some() && plain.get("asks").is_some();
        // detect update messages
        let is_update_message = plain.get("update").is_some();

        if is_checkpoint_message {
            let checkpoint: OrderbookCheckpoint = serde_json::from_value(plain.clone()).expect("");

            for bid in checkpoint.bids {
                let price = SellPrice {
                    price: bid[0],
                    quantity: bid[1],
                    // TODO derive from slot
                    approx_timestamp: Instant::now(),
                };
                orderbook.update_bid_price(price.price, price.quantity);
                let mut lock = highest_bid_price.write().await;
                *lock = orderbook.get_highest_bid_price();
            }

            for ask in checkpoint.asks {
                let price = SellPrice {
                    price: ask[0],
                    quantity: ask[1],
                    // TODO derive from slot
                    approx_timestamp: Instant::now(),
                };
                orderbook.update_ask_price(price.price, price.quantity);
                let mut lock = lowest_ask_price.write().await;
                *lock = orderbook.get_lowest_ask_price();
            }
        }

        if is_update_message {
            let update: OrderbookUpdate = serde_json::from_value(plain.clone()).expect(format!("Can't convert json <{}>", msg).as_str());

            debug!("update({:?}): {:?}", update.slot, update.update);
            for data in update.update {
                let price = SellPrice {
                    price: data[0],
                    quantity: data[1],
                    approx_timestamp: Instant::now(),
                };
                if update.side == OrderbookSide::Bid {
                    orderbook.update_bid_price(price.price, price.quantity);
                    let mut lock = highest_bid_price.write().await;
                    *lock = Some(price.price);
                }
                if update.side == OrderbookSide::Ask {
                    orderbook.update_ask_price(price.price, price.quantity);
                    let mut lock = lowest_ask_price.write().await;
                    *lock = Some(price.price);
                }

                // TODO remove
                orderbook.dump();
                // sell_price_xwrite.send(price).unwrap();
            }

        }

    }


}

