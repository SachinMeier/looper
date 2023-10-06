use bdk::{
    bitcoin::{
        blockdata::{opcodes, script},
        hashes::{hex::ToHex, sha256::Hash as Sha256, sha256d::Hash as Sha256d, Hash},
        schnorr::{UntweakedKeyPair, UntweakedPublicKey},
        secp256k1::{rand::thread_rng, PublicKey, Secp256k1, SecretKey},
        util::{
            bip32::{ExtendedPrivKey, ExtendedPubKey},
            key::{KeyPair, PrivateKey, XOnlyPublicKey},
            taproot,
            taproot::{TaprootBuilder, TaprootSpendInfo},
        },
        Network,
    },
    blockchain::{ConfigurableBlockchain, RpcBlockchain, RpcConfig},
    database::SqliteDatabase,
    Balance, SyncOptions,
};

use http::status::StatusCode;
use std::str::FromStr;

use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

use crate::lnd::client::LNDGateway;
use crate::services::errors::{LooperError, LooperErrorResponse};
use crate::settings;
use crate::wallet::LooperWallet;

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutRequest {
    pub pubkey: String,
    pub amount: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct TaprootScriptInfo {
    pub external_key: String,
    pub internal_key: String,
    pub internal_key_tweak: String,
    pub tree: Vec<String>,
}

fn new_taproot_script_info(tsi: TaprootSpendInfo, tweak: SecretKey) -> TaprootScriptInfo {
    TaprootScriptInfo {
        external_key: tsi.output_key().to_string(),
        internal_key: tsi.internal_key().to_string(),
        internal_key_tweak: hex::encode(tweak.secret_bytes()),
        tree: tree_to_vec(tsi),
    }
}

fn tree_to_vec(tsi: TaprootSpendInfo) -> Vec<String> {
    let mut vec: Vec<String> = vec![];
    let iter = tsi.as_script_map().into_iter();

    for ((script, _version), _) in iter {
        vec.push(script.to_hex());
    }

    return vec;
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutInfo {
    pub fee: i64,
    pub loop_hash: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutResponse {
    pub invoice: String,
    pub pubkey: String,
    pub tweak: String,
    pub taproot_script_info: TaprootScriptInfo,
    pub loop_info: LoopOutInfo,
}

pub struct LoopOutConfig {
    pub min_amount: i64,
    pub max_amount: i64,
    pub cltv_timeout: u64,
    pub fee_pct: i64,
}

// lazy_static::lazy_static! {
//     static ref LOS: LoopOutService = LoopOutService::new();
// }

pub struct LoopOutService {
    cfg: LoopOutConfig,
}

impl LoopOutService {
    fn new() -> Self {
        let cfg = settings::build_config().unwrap();
        Self {
            cfg: LoopOutConfig {
                min_amount: cfg.get_int("loopout.min").unwrap(),
                max_amount: cfg.get_int("loopout.max").unwrap(),
                cltv_timeout: cfg.get_int("loopout.cltv").unwrap().try_into().unwrap(),
                fee_pct: cfg.get_int("loopout.fee").unwrap(),
            },
        }
    }

    pub fn handle_loop_out_request(
        &self,
        req: LoopOutRequest,
    ) -> Result<LoopOutResponse, LooperErrorResponse> {
        self.validate_request(req)?;

        let buyer_pubkey: PublicKey = PublicKey::from_str(&req.pubkey).unwrap();
        let fee = self.calculate_fee(req.amount);
        let invoice_amount = req.amount + fee;
        // create new pubkey
        let looper_pubkey: PublicKey = PublicKey::from_str("todo implement me").unwrap();

        let invoice = LNDGateway::add_invoice(invoice_amount, self.cfg.cltv_timeout).unwrap();

        let mut payhash_bytes = [0u8; 32];
        hex::decode_to_slice(&invoice.payment_hash, &mut payhash_bytes as &mut [u8]).unwrap();

        let cltv_timeout: i64 = self.cfg.cltv_timeout.try_into().unwrap();
        let (tr, tweak) =
            LooperWallet::new_htlc(buyer_pubkey, looper_pubkey, &payhash_bytes, cltv_timeout);

        let taproot_script_info = new_taproot_script_info(tr, tweak);

        // convert tr, tweak to scriptInfo & string respectively

        let loop_info = LoopOutInfo {
            fee,
            loop_hash: invoice.payment_hash,
        };

        let resp = LoopOutResponse {
            invoice: invoice.invoice,
            pubkey: hex::encode(looper_pubkey.serialize()),
            tweak: hex::encode(tweak.secret_bytes()),
            taproot_script_info,
            loop_info,
        };

        return Ok(resp);
    }

    fn calculate_fee(&self, amount: i64) -> i64 {
        return amount * self.cfg.fee_pct / 100;
    }

    fn validate_request(&self, req: LoopOutRequest) -> Result<(), LooperErrorResponse> {
        self.validate_amount(req.amount)?;
        self.validate_pubkey(req.pubkey)?;
        Ok(())
    }

    fn validate_amount(&self, amount: i64) -> Result<(), LooperErrorResponse> {
        if amount < self.cfg.min_amount {
            return Err(LooperErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "amount too low".to_string(),
                "amount".to_string(),
            ));
        }

        if amount > self.cfg.max_amount {
            return Err(LooperErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "amount too high".to_string(),
                "amount".to_string(),
            ));
        }
        Ok(())
    }

    fn validate_pubkey(&self, pubkey_str: String) -> Result<(), LooperErrorResponse> {
        match PublicKey::from_str(&pubkey_str) {
            Ok(_) => Ok(()),

            Err(_) => Err(LooperErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "invalid pubkey".to_string(),
                "pubkey".to_string(),
            )),
        }
    }
}
