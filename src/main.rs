#[macro_use]
extern crate rocket;

// #[macro_use]
// extern crate diesel;

mod api;
mod db;
pub mod lnd;
pub mod mempool;
pub mod models;
mod schema;
mod services;
pub mod settings;
mod utils;
pub mod wallet;

use crate::lnd::client::LNDGateway;
// use bdk::bitcoin::secp256k1::PublicKey;
use db::DB;
use std::io::{self, BufRead};
// use std::str::FromStr;

// use rand::Rng;

#[tokio::main]
async fn main() {
    run().await;
}

async fn run() {
    let cfg = settings::build_config().unwrap();

    let db = DB::new(&cfg);

    let migration_conn = &mut db.get_conn().unwrap();
    db::run_migrations(migration_conn).unwrap();

    let wallet = wallet::LooperWallet::new(&cfg).unwrap();

    let lndg = LNDGateway::new().await.unwrap();

    let loopout_svc = services::loop_out::LoopOutService::new(&cfg, db, wallet, lndg).unwrap();

    let server = api::server::LooperServer::new(loopout_svc);
    server.start();

    let stdin = io::stdin();
    let _line = stdin.lock().lines().next().unwrap().unwrap();
}
