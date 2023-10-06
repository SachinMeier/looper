// mod db;
mod db;
mod lnd;
mod pgdb;
mod settings;
mod utils;
mod wallet;
// mod api;
mod services;

#[macro_use]
extern crate rocket;

use bdk::bitcoin::secp256k1::PublicKey;
use db::Db;
use lnd::{client, client::LNDGateway};
// use wallet::LooperWallet;
use std::str::FromStr;

use rand::Rng;

fn main() {
    startup();

    // let info = LNDGateway::get_info();
    // println!("{:?}", info);
    let invoice = LNDGateway::add_invoice(100, 400);
    println!("{:?}", invoice);
    // code goes here
    // play();
    // api::start();

    // println!("hit enter to exit");
    // let mut input = String::new();
    // std::io::stdin().read_line(&mut input).unwrap();

    shutdown();
}

fn script_build() {
    let pk1_str = "02fcba7ecf41bc7e1be4ee122d9d22e3333671eb0a3a87b5cdf099d59874e1940f";
    let pk1 = PublicKey::from_str(&pk1_str).unwrap();

    let pk2_str = "03aaaaaacf41bc7e1be4ee122d9d22e3333671eb0a3a87b5cdf099d59874e1940f";
    let pk2 = PublicKey::from_str(&pk2_str).unwrap();

    let mut rng = rand::thread_rng();
    let mut payment_hash: [u8; 32] = [0; 32];
    for i in 0..32 {
        payment_hash[i] = rng.gen::<u8>();
    }
}

fn startup() {
    let cfg = settings::build_config().unwrap();
    settings::init_logging();

    Db::start();
    Db::migrate().unwrap();

    LNDGateway::start();
}

fn shutdown() {
    // LNDGateway::stop();
    Db::stop();
}
