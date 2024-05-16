extern crate orderbook;

use orderbook::binance::Configuration;
use orderbook::binance::Manager;
use orderbook::tokens;
use orderbook::Spot;
use tracing;
use tracing_subscriber;

const BTCUSDT: Spot = Spot {
    base: tokens::USDT,
    quote: tokens::BTC,
};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    tracing::info!("Starting the manager");
    let mut binance_manager = Manager::new(Configuration::new());
    binance_manager.subscribe(&BTCUSDT).await;

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
