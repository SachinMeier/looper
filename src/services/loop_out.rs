use bdk::{
    bitcoin::{
        bip32::{ChildNumber, ExtendedPrivKey, ExtendedPubKey},
        blockdata::{
            locktime::absolute::LockTime, opcodes, script, script::PushBytes, transaction::OutPoint,
        },
        hashes::{sha256::Hash as Sha256, sha256d::Hash as Sha256d, Hash},
        secp256k1::{
            self, rand::thread_rng, KeyPair, Parity, PublicKey, Secp256k1, SecretKey,
            XOnlyPublicKey,
        },
        taproot,
        taproot::{TaprootBuilder, TaprootSpendInfo},
        Address, Network, ScriptBuf,
    },
    blockchain::{ConfigurableBlockchain, RpcBlockchain, RpcConfig},
    database::SqliteDatabase,
    descriptor::Descriptor,
    wallet::{wallet_name_from_descriptor, AddressIndex, AddressInfo},
    Balance,
    KeychainKind::{self, External, Internal},
    SyncOptions, Wallet,
};
use hex::ToHex;
use std::mem;

use tokio::sync::Mutex;

use http::status::StatusCode;
use std::str::FromStr;

use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

use crate::{
    db::DB,
    lnd::{
        self,
        client::{AddInvoiceResp, LNDGateway},
    },
    models::{FullLoopOutData, NewInvoice},
    services::errors::{LooperError, LooperErrorResponse},
    settings,
    wallet::LooperWallet,
};

pub const LoopOutStateInitiated: &str = "INITIATED";
pub const LoopOutStateConfirmed: &str = "CONFIRMED";
pub const LoopOutStateClaimed: &str = "CLAIMED";
pub const LoopOutStateFailed: &str = "TIMEOUT";

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

fn new_taproot_script_info(tsi: &TaprootSpendInfo, tweak: SecretKey) -> TaprootScriptInfo {
    TaprootScriptInfo {
        external_key: tsi.output_key().to_string(),
        internal_key: tsi.internal_key().to_string(),
        internal_key_tweak: hex::encode(tweak.secret_bytes()),
        tree: tree_to_vec(tsi),
    }
}

fn tree_to_vec(tsi: &TaprootSpendInfo) -> Vec<String> {
    let mut vec: Vec<String> = vec![];
    let iter = tsi.as_script_map().iter();

    for ((script, _version), _) in iter {
        vec.push(script.to_hex_string());
    }

    vec
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
    pub error: Option<LooperError>,
}

pub struct LoopOutConfig {
    pub min_amount: i64,
    pub max_amount: i64,
    // cltv_delta is the different between the blockheight the invoice expires and the blockheight
    // the utxo timeout script is available
    pub cltv_delta: u64,
    pub fee_pct: i64,
}

pub struct LoopOutService {
    cfg: LoopOutConfig,
    secp256k1: Secp256k1<secp256k1::All>,
    network: Network,
    db: DB,
    wallet: Mutex<LooperWallet>,
    lnd_gateway: Mutex<LNDGateway>,
}

impl LoopOutService {
    pub fn new(
        cfg: &settings::Config,
        db: DB,
        wallet: LooperWallet,
        lnd_gateway: LNDGateway,
    ) -> Self {
        Self {
            cfg: LoopOutConfig {
                min_amount: cfg.get_int("loopout.min").unwrap(),
                max_amount: cfg.get_int("loopout.max").unwrap(),
                cltv_delta: cfg.get_int("loopout.cltv").unwrap().try_into().unwrap(),
                fee_pct: cfg.get_int("loopout.fee").unwrap(),
            },
            db,
            secp256k1: Secp256k1::new(),
            network: wallet.get_network(),
            wallet: Mutex::new(wallet),
            lnd_gateway: Mutex::new(lnd_gateway),
        }
    }

    pub fn get_loop_out_request(&self, payment_hash: String) -> LoopOutResponse {
        // TODO: err handle
        let data = self.db.get_full_loop_out(payment_hash).unwrap();

        map_loop_out_data_to_response(data)
    }

    fn map_loop_out_data_to_response(data: models::FullLoopOutData) -> LoopOutResponse {
        let resp = LoopOutResponse {
            invoice: data.invoice.payment_request,
            address: data.script.address,
            looper_pubkey: data.loop_out.local_pubkey,
            txid: data.utxo.txid,
            vout: data.utxo.vout,
            taproot_script_info: TaprootScriptInfo {
                external_key: data.script.external_key,
                internal_key: data.script.internal_key,
                internal_key_tweak: data.script.internal_key_tweak,
                tree: data.script.tree,
            },
            loop_info: LoopOutInfo {
                // TODO: calculate fee or store it in db?
                fee: 0,
                loop_hash: data.invoice.payment_hash,
                // TODO: fix
                cltv_expiry: data.script.cltv_expiry,
            },
            error: None,
        };

        resp
    }

    pub async fn handle_loop_out_request(&self, req: LoopOutRequest) -> (LoopOutResponse, u32) {
        self.validate_request(&req).unwrap();
        log::info!("validated request");
        let buyer_pubkey: XOnlyPublicKey = XOnlyPublicKey::from_str(&req.pubkey).unwrap();
        let fee = self.calculate_fee(&req.amount);
        let invoice_amount = req.amount + fee;

        // Lock wallet here and get all necessary info
        let wallet = self.wallet.lock().await;
        // create new pubkey
        let (looper_pubkey, looper_pubkey_idx) = (*wallet).new_pubkey();
        let curr_height = (*wallet).get_height().unwrap();
        log::info!("curr_height: {}", curr_height);
        let cltv_delta: u32 = self.cfg.cltv_delta.try_into().unwrap();
        // TODO: currently unused. bad
        let cltv_expiry = curr_height + cltv_delta;
        mem::drop(wallet);
        // Unlock wallet

        let invoice = self
            .add_invoice(invoice_amount, cltv_expiry as u64)
            .await
            .unwrap();

        let mut payhash_bytes = [0u8; 32];
        hex::decode_to_slice(&invoice.payment_hash, &mut payhash_bytes as &mut [u8]).unwrap();

        let (tr, tweak) =
            LooperWallet::new_htlc(buyer_pubkey, looper_pubkey, &payhash_bytes, cltv_expiry);

        let taproot_script_info = new_taproot_script_info(&tr, tweak);

        let loop_info = LoopOutInfo {
            fee,
            loop_hash: invoice.payment_hash,
            cltv_expiry,
        };

        let address = self.p2tr(&tr);

        let txid = self
            .build_tx_to_address(&address.to_string(), invoice_amount as u64)
            .await
            .unwrap();

        (
            LoopOutResponse {
                invoice: invoice.invoice,
                address: address.to_string(),
                looper_pubkey: hex::encode(looper_pubkey.serialize()),
                txid: format!("{:x}", txid.as_raw_hash().forward_hex()),
                vout: 0,
                taproot_script_info,
                loop_info,
                error: None,
            },
            looper_pubkey_idx,
        )
    }

    async fn add_invoice(
        &self,
        amount: i64,
        cltv_expiry: u64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
        log::info!("adding invoice...");
        let lndg = self.lnd_gateway.lock().await;
        let invoice = lndg.add_invoice(amount, cltv_expiry).await.unwrap();
        mem::drop(lndg);

        let new_invoice = NewInvoice {
            // TODO: placeholder. FIXME
            loop_out_id: 1,
            payment_request: &invoice.invoice,
            payment_hash: &invoice.payment_hash,
            payment_preimage: Some(&invoice.preimage),
            amount,
            state: lnd::InvoiceOpen.to_string(),
        };

        match self.db.insert_invoice(new_invoice) {
            Ok(_) => log::info!("inserted invoice into db"),
            Err(e) => log::error!("error inserting invoice into db: {}", e),
        }

        Ok(invoice)
    }

    async fn build_tx_to_address(
        &self,
        address: &str,
        amount: u64,
    ) -> Result<bitcoin::Txid, LooperErrorResponse> {
        let wallet = self.wallet.lock().await;
        let txid = (*wallet).send_to_address(address, amount)?;
        mem::drop(wallet);
        Ok(txid)
    }

    fn p2tr(&self, tr: &TaprootSpendInfo) -> Address {
        Address::p2tr(
            &self.secp256k1,
            tr.internal_key(),
            tr.merkle_root(),
            self.network,
        )
    }

    fn calculate_fee(&self, amount: &i64) -> i64 {
        amount * self.cfg.fee_pct / 100
    }

    fn validate_request(&self, req: &LoopOutRequest) -> Result<(), LooperErrorResponse> {
        self.validate_amount(req.amount)?;
        self.validate_pubkey(&req.pubkey)?;
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

    pub fn validate_pubkey(&self, pubkey_str: &str) -> Result<(), LooperErrorResponse> {
        match XOnlyPublicKey::from_str(pubkey_str) {
            Ok(_) => Ok(()),

            Err(_) => Err(LooperErrorResponse::new(
                StatusCode::BAD_REQUEST,
                "invalid pubkey".to_string(),
                "pubkey".to_string(),
            )),
        }
    }
}
