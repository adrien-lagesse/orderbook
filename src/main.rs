mod binance;

use binance::Manager;
use std::{thread::sleep, time};
use tokio;

mod core;

#[tokio::main]
async fn main() {
    let mut m = Manager::new();
    let symbol = "btcusdt";
    m.suscribe(symbol).await;
    for _ in 0..6000 {
        sleep(time::Duration::from_millis(100));
        println!(
            "PRICE {:.3}            SPREAD {:.5}    IMBALANCE {:.3}         BID ({:.3}, {:.3})  ASK ({:.3}, {:.3})",
            m.get_price(symbol).await.unwrap(),
            m.get_spread(symbol).await.unwrap()/m.get_price(symbol).await.unwrap()*10000.,
            m.get_imbalance(symbol).await.unwrap(),
            m.best_bid(symbol).await.unwrap().0,
            m.best_bid(symbol).await.unwrap().1,
            m.best_ask(symbol).await.unwrap().0,
            m.best_ask(symbol).await.unwrap().1
        )
    }
    m.unsubscribe(symbol).await;
    m.close().await;
}
