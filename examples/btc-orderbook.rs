extern crate orderbook;

use orderbook::binance::Configuration;
use orderbook::binance::Manager;
use orderbook::tokens;
use orderbook::Spot;

const BTCUSDT: Spot = Spot {
    base: tokens::USDT,
    quote: tokens::BTC,
};

#[tokio::main]
async fn main() {
    let mut binance_manager = Manager::new(Configuration::new());
    binance_manager.subscribe(&BTCUSDT).await;

    for _ in 0..6000 {
        std::thread::sleep(std::time::Duration::from_millis(100));
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
    binance_manager.unsubscribe(&BTCUSDT).await;
}
