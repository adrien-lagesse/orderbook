extern crate orderbook;

use std::{thread::sleep, time};

use orderbook::binance::Configuration;
use orderbook::binance::Manager;
use orderbook::tokens;
use orderbook::Spot;

#[tokio::main]
async fn main() {
    let mut binance_manager = Manager::new(Configuration::new());
    const SOLUSDT : Spot = Spot {
        base: tokens::USDT,
        quote: tokens::SOL,
    };
    const ETHUSDT : Spot = Spot {
        base: tokens::USDT,
        quote: tokens::ETH,
    };
    const BTCUSDT : Spot = Spot {
        base: tokens::USDT,
        quote: tokens::BTC,
    };
    let begin = time::Instant::now();
    binance_manager.subscribe(&SOLUSDT).await;
    binance_manager.subscribe(&BTCUSDT).await;
    binance_manager.subscribe(&ETHUSDT).await;
    println!("{}", (time::Instant::now() - begin).as_millis());

    for _ in 0..6000 {
        sleep(time::Duration::from_millis(100));
        println!(
            "PRICE {:.3}            SPREAD {:.5}    IMBALANCE {:.3}         BID ({:.3}, {:.3})  ASK ({:.3}, {:.3})",
            binance_manager.get_price(&SOLUSDT).await.unwrap(),
            binance_manager.get_spread(&SOLUSDT).await.unwrap()/binance_manager.get_price(&SOLUSDT).await.unwrap()*10000.,
            binance_manager.get_imbalance(&SOLUSDT).await.unwrap(),
            binance_manager.best_bid(&SOLUSDT).await.unwrap().0,
            binance_manager.best_bid(&SOLUSDT).await.unwrap().1,
            binance_manager.best_ask(&SOLUSDT).await.unwrap().0,
            binance_manager.best_ask(&SOLUSDT).await.unwrap().1
        )
    }
    binance_manager.unsubscribe(&SOLUSDT).await;
}
