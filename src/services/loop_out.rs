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
    models::{self, FullLoopOutData, Invoice, NewInvoice, NewScript, NewUTXO, Script, UTXO},
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
    // cltv_delta is how many blocks before the UTXO's timelock expires
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
        // TODO: use LoopOutSvcConfig
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

        Self::map_loop_out_data_to_response(data)
    }

    fn map_loop_out_data_to_response(data: models::FullLoopOutData) -> LoopOutResponse {
        let resp = LoopOutResponse {
            invoice: data.invoice.payment_request,
            address: data.script.address,
            looper_pubkey: data.script.local_pubkey,
            // TODO: resolve this. make UTXOs not a vec, or sth
            txid: data.utxos[0].txid,
            vout: data.utxos[0].vout as u32,
            // TOOD: breakout TaprootScriptInfo From Script function
            taproot_script_info: TaprootScriptInfo {
                external_key: data.script.external_tapkey,
                internal_key: data.script.internal_tapkey,
                internal_key_tweak: data.script.internal_tapkey_tweak,
                // TODO: fix Vec<String> vs Vec<Option<String>>
                tree: data.script.tree,
            },
            loop_info: LoopOutInfo {
                // TODO: calculate fee or store it in db?
                fee: 0,
                loop_hash: data.invoice.payment_hash,
                // TODO: fix
                cltv_expiry: data.script.cltv_expiry as u32,
            },
            error: None,
        };

        resp
    }

    pub async fn handle_loop_out_request(&self, req: LoopOutRequest) -> LoopOutResponse {
        self.validate_request(&req).unwrap();
        log::info!("validated request");
        let buyer_pubkey: XOnlyPublicKey = XOnlyPublicKey::from_str(&req.pubkey).unwrap();
        let fee = self.calculate_fee(&req.amount);
        let invoice_amount = req.amount + fee;

        let loop_out = self.add_loop_out();

        let invoice = self
            .add_invoice(&loop_out.id, invoice_amount)
            .await
            .unwrap();

        let script = self
            .add_onchain_htlc(&loop_out.id, &buyer_pubkey, &invoice.payment_hash)
            .await;

        let utxo = self
            .add_utxo_to_htlc(script.id, &script.address, req.amount as u64)
            .await;

        let full_loop_out_data = DB::new_full_loop_out_data(loop_out, invoice, script, vec![utxo]);

        Self::map_loop_out_data_to_response(full_loop_out_data)
    }

    fn add_loop_out(&self) -> models::LoopOut {
        let new_loop_out = models::NewLoopOut {
            state: LoopOutStateInitiated.to_string(),
        };

        self.db.insert_loop_out(new_loop_out).unwrap()
    }

    async fn add_invoice(
        &self,
        loop_out_id: &i64,
        amount: i64,
    ) -> Result<Invoice, diesel::result::Error> {
        log::info!("adding invoice...");
        let lndg = self.lnd_gateway.lock().await;
        let invoice = lndg.add_invoice(amount).await.unwrap();
        mem::drop(lndg);

        let new_invoice = NewInvoice {
            loop_out_id: *loop_out_id,
            payment_request: &invoice.invoice,
            payment_hash: &invoice.payment_hash,
            payment_preimage: Some(&invoice.preimage),
            amount,
            state: lnd::InvoiceOpen.to_string(),
        };

        self.db.insert_invoice(new_invoice)
    }

    async fn add_onchain_htlc(
        &self,
        loop_out_id: &i64,
        buyer_pubkey: &XOnlyPublicKey,
        payment_hash: &String,
    ) -> Script {
        // Lock wallet here and get all necessary info
        let wallet = self.wallet.lock().await;
        // create new pubkey
        let (looper_pubkey, looper_pubkey_idx) = (*wallet).new_pubkey();
        // TODO: sync here to get proper height?
        let curr_height = (*wallet).get_height().unwrap();
        log::info!("curr_height: {}", curr_height);
        let cltv_delta: u32 = self.cfg.cltv_delta.try_into().unwrap();
        // TODO: currently unused. bad
        let cltv_expiry = curr_height + cltv_delta;
        mem::drop(wallet);
        // Unlock wallet

        let mut payhash_bytes = [0u8; 32];
        hex::decode_to_slice(&payment_hash, &mut payhash_bytes as &mut [u8]).unwrap();

        let (tr, tweak) =
            LooperWallet::new_htlc(*buyer_pubkey, looper_pubkey, &payhash_bytes, cltv_expiry);

        let address = self.p2tr_address(&tr);

        // TODO: factor into function
        let script = NewScript {
            loop_out_id: *loop_out_id,
            address: &address.to_string(),
            external_tapkey: &tr.output_key().to_string(),
            internal_tapkey: &tr.internal_key().to_string(),
            internal_tapkey_tweak: &hex::encode(tweak.secret_bytes()),
            tree: tree_to_vec(&tr),
            cltv_expiry: cltv_expiry.try_into().unwrap(),
            remote_pubkey: buyer_pubkey.to_string(),
            local_pubkey: looper_pubkey.to_string(),
            local_pubkey_index: looper_pubkey_idx as i32,
        };
        self.db.insert_script(script).unwrap()
    }

    async fn add_utxo_to_htlc(&self, script_id: i64, address: &str, amount: u64) -> UTXO {
        let txid = self.build_tx_to_address(address, amount).await.unwrap();

        let utxo = NewUTXO {
            txid: &txid.to_string(),
            // TODO: fix
            vout: 0,
            amount: amount.try_into().unwrap(),
            script_id,
        };
        let utxo = self.db.insert_utxo(utxo).unwrap();

        utxo
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

    fn p2tr_address(&self, tr: &TaprootSpendInfo) -> Address {
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
