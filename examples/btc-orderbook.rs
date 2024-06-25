extern crate orderbook;

use orderbook::binance::Configuration;
use orderbook::binance::Manager;
use orderbook::tokens;
use orderbook::Spot;
use tracing;
use tracing_subscriber;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::util::SubscriberInitExt;
use uuid::Uuid;

const BTCUSDT: Spot = Spot {
    base: tokens::USDT,
    quote: tokens::BTC,
};

const ETHUSDT: Spot = Spot {
    base: tokens::USDT,
    quote: tokens::ETH,
};

#[tokio::main]
async fn main() {
    let _subscriber = tracing_subscriber::fmt::init();
        // // .json()
        // // .flatten_event(true)
        // .finish()
        // .try_init()
        // .unwrap();


    let mut binance_manager = Manager::new(Configuration::new());
    binance_manager.subscribe(&BTCUSDT).await.unwrap();
    binance_manager.subscribe(&ETHUSDT).await.unwrap();

    for _ in 0..24 {
        std::thread::sleep(std::time::Duration::from_millis(5000));
        println!(
            "PRICE {:.3}            SPREAD {:.5}    IMBALANCE {:.3}         BID ({:.3}, {:.3})  ASK ({:.3}, {:.3})",
            binance_manager.get_price(&BTCUSDT).await.unwrap(),
            binance_manager.get_spread(&BTCUSDT).await.unwrap()/binance_manager.get_price(&BTCUSDT).await.unwrap()*10000.,
            binance_manager.get_imbalance(&BTCUSDT).await.unwrap(),
            binance_manager.best_bid(&BTCUSDT).await.unwrap().0,
            binance_manager.best_bid(&BTCUSDT).await.unwrap().1,
            binance_manager.best_ask(&BTCUSDT).await.unwrap().0,
            binance_manager.best_ask(&BTCUSDT).await.unwrap().1
        )
    }
}
