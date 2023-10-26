#[macro_use]
extern crate rocket;

#[macro_use]
extern crate diesel;

mod api;
mod db;
pub mod lnd;
pub mod models;
mod schema;
mod services;
pub mod settings;
mod utils;
pub mod wallet;

use crate::lnd::client::LNDGateway;
use bdk::bitcoin::secp256k1::PublicKey;
use db::DB;
use std::io::{self, BufRead};
use std::str::FromStr;

use rand::Rng;

#[tokio::main]
async fn main() {
    test_full_db_loop_out();
}

// fn test_insert_invoice() {
//     let cfg = settings::build_config().unwrap();

//     let db = DB::new(&cfg);

//     let new_invoice = models::NewInvoice {
//         payment_request: "invoice",
//         payment_hash: "hash",
//         payment_preimage: None,
//         amount: 100,
//         state: lnd::InvoiceOpen.to_string(),
//     };

//     let invoice = db.insert_invoice(new_invoice).unwrap();

//     let mut fetched_invoice = db.get_invoice(invoice.id).unwrap();
//     print!("{:?}", fetched_invoice);
//     fetched_invoice.payment_preimage = Some("preimage".to_string());

//     let updated_invoice = db.update_invoice(fetched_invoice).unwrap();
//     print!("{:?}", updated_invoice);
// }

// fn test_db_scripts() {
//     let cfg = settings::build_config().unwrap();

//     let db = DB::new(&cfg);

//     let new_script = models::NewScript {
//         address: "address",
//         external_key: "ext",
//         internal_key: "int",
//         internal_key_tweak: "tweak",
//         tree: vec!["hashcontract".to_string(), "timeout".to_string()],
//     };

//     let script = db.insert_script(new_script).unwrap();

//     let fetched_script = db.get_script(script.id).unwrap();
//     print!("{:?}", fetched_script);
// }

fn test_full_db_loop_out() {
    let cfg = settings::build_config().unwrap();

    let db = DB::new(&cfg);

    let new_loop_out = models::NewLoopOut {
        state: services::loop_out::LoopOutStateInitiated.to_string(),
        remote_pubkey: "remote_pubkey".to_string(),
        local_pubkey: "local_pubkey".to_string(),
        local_pubkey_index: 0,
        cltv_timeout: 100,
    };

    let loop_out = db.insert_loop_out(new_loop_out).unwrap();

    let new_invoice = models::NewInvoice {
        loop_out_id: loop_out.id,
        payment_request: "invoice",
        payment_hash: "hash",
        payment_preimage: None,
        amount: 100,
        state: lnd::InvoiceOpen.to_string(),
    };

    let invoice = db.insert_invoice(new_invoice).unwrap();

    let new_script = models::NewScript {
        loop_out_id: loop_out.id,
        address: "address2",
        external_key: "ext2",
        internal_key: "int2",
        internal_key_tweak: "tweak2",
        tree: vec!["hashcontract".to_string(), "timeout".to_string()],
    };

    let script = db.insert_script(new_script).unwrap();

    let new_utxo = models::NewUTXO {
        script_id: script.id,
        txid: "txid",
        vout: 0,
        amount: 100,
    };

    let utxo = db.insert_utxo(new_utxo).unwrap();

    let full_loop_out_data = db.get_full_loop_out("hash".to_string()).unwrap();

    print!("{:?}", full_loop_out_data);
}

// fn test_db_utxos() {
//     let cfg = settings::build_config().unwrap();

//     let db = DB::new(&cfg);

//     let new_script = models::NewScript {
//         address: "address2",
//         external_key: "ext2",
//         internal_key: "int2",
//         internal_key_tweak: "tweak2",
//         tree: vec!["hashcontract".to_string(), "timeout".to_string()],
//     };

//     let script = db.insert_script(new_script).unwrap();

//     let new_utxo = models::NewUTXO {
//         txid: "txid",
//         vout: 0,
//         amount: 100,
//         script_id: script.id,
//     };

//     let utxo = db.insert_utxo(new_utxo).unwrap();

//     let fetched_utxo = db.get_utxo(utxo.id).unwrap();
//     print!("{:?}", fetched_utxo);
// }

async fn run() {
    let cfg = settings::build_config().unwrap();

    let db = DB::new(&cfg);

    let wallet = wallet::LooperWallet::new(&cfg);

    let lndg = LNDGateway::new().await;

    let loopout_svc = services::loop_out::LoopOutService::new(&cfg, db, wallet, lndg);

    let server = api::LooperServer::new(loopout_svc);
    server.start();

    let stdin = io::stdin();
    let _line = stdin.lock().lines().next().unwrap().unwrap();
}
