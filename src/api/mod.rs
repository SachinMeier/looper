use crate::{
    models::{FullLoopOutData, Script},
    services::loop_out,
};
use bdk::bitcoin::{secp256k1::SecretKey, taproot::TaprootSpendInfo};
use rocket::serde::{Deserialize, Serialize};

pub mod errors;
pub mod server;

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutRequest {
    pub pubkey: String,
    pub amount: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutInfo {
    pub fee: i64,
    pub loop_hash: String,
    pub cltv_expiry: u32,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutResponse {
    pub invoice: String,
    pub address: String,
    pub looper_pubkey: String,
    pub txid: String,
    pub vout: u32,
    pub taproot_script_info: TaprootScriptInfo,
    pub loop_info: LoopOutInfo,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct TaprootScriptInfo {
    pub external_key: String,
    pub internal_key: String,
    pub internal_key_tweak: String,
    pub tree: Vec<String>,
}

#[allow(dead_code)]
fn new_taproot_script_info(tsi: &TaprootSpendInfo, tweak: SecretKey) -> TaprootScriptInfo {
    TaprootScriptInfo {
        external_key: tsi.output_key().to_string(),
        internal_key: tsi.internal_key().to_string(),
        internal_key_tweak: hex::encode(tweak.secret_bytes()),
        tree: loop_out::tree_to_vec(tsi),
    }
}

fn script_to_taproot_script_info(script: Script) -> TaprootScriptInfo {
    let mut tree: Vec<String> = vec![];
    for t in script.tree {
        match t {
            None => {}
            Some(t) => tree.push(t),
        }
    }

    TaprootScriptInfo {
        external_key: script.external_tapkey,
        internal_key: script.internal_tapkey,
        internal_key_tweak: script.internal_tapkey_tweak,
        tree,
    }
}

fn map_loop_out_data_to_response(data: FullLoopOutData) -> LoopOutResponse {
    // TODO: must extract these out here to allow script_to_taproot_script_info to move script
    let cltv_expiry = data.script.cltv_expiry as u32;
    let address = data.script.address.clone();
    let looper_pubkey = data.script.local_pubkey.clone();

    LoopOutResponse {
        invoice: data.invoice.payment_request,
        address,
        looper_pubkey,
        txid: data.utxo.txid,
        vout: data.utxo.vout as u32,
        taproot_script_info: script_to_taproot_script_info(data.script),
        loop_info: LoopOutInfo {
            // TODO: calculate fee or store it in db?
            fee: 0,
            loop_hash: data.invoice.payment_hash,
            cltv_expiry,
        },
    }
}
