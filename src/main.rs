// mod api;
mod db;
pub mod lnd;
mod pgdb;
mod services;
pub mod settings;
mod utils;
pub mod wallet;

#[macro_use]
extern crate rocket;

use crate::lnd::client::LNDGateway;
use bdk::bitcoin::secp256k1::PublicKey;
use db::Db;
use std::io::{self, BufRead};
use std::str::FromStr;

use rand::Rng;
fn main() {
    let cfg = settings::build_config().unwrap();
    startup();

    let wallet = wallet::LooperWallet::new(&cfg);

    // let addr = wallet.new_address();
    // log::info!("addr: {:?}", addr);

    // log::info!("balance: {:?}", wallet.get_balance());

    let loopout_svc = services::loop_out::LoopOutService::new(&cfg, wallet);

    // let server = api::LooperServer::new(loopout_svc);
    // server.start();

    let req = services::loop_out::LoopOutRequest {
        pubkey: "146846eeb5a7533abb594ba734bc243fc7b6349499b8311c8fc13b0112ba8a77".to_string(),
        amount: 1_000_000,
    };

    let (resp, idx) = loopout_svc.handle_loop_out_request(req);

    log::info!("{:?}", resp);

    // let stdin = io::stdin();
    // let _line = stdin.lock().lines().next().unwrap().unwrap();

    shutdown();
}

fn startup() {
    settings::init_logging();

    Db::start();
    Db::migrate().unwrap();

    LNDGateway::start();
}

fn shutdown() {
    // LNDGateway::stop();
    Db::stop();
}

fn example() {
    let cfg = settings::build_config().unwrap();
    startup();

    let wallet = wallet::LooperWallet::new(&cfg);

    let addr = wallet.new_address();
    log::info!("addr: {:?}", addr);

    let stdin = io::stdin();
    let _line = stdin.lock().lines().next().unwrap().unwrap();

    wallet.sync().unwrap();

    log::info!("balance: {:?}", wallet.get_balance());

    let loopout_svc = services::loop_out::LoopOutService::new(&cfg, wallet);

    let req = services::loop_out::LoopOutRequest {
        pubkey: "02fcba7ecf41bc7e1be4ee122d9d22e3333671eb0a3a87b5cdf099d59874e1940f".to_string(),
        amount: 1_000_000,
    };

    log::info!("req: {:?}", req);
    let res = loopout_svc.handle_loop_out_request(req);
    log::info!("{:?}", res);

    shutdown();
}
