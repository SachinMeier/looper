use bdk::{
    bitcoin::{
        bip32::{ChildNumber, ExtendedPrivKey, ExtendedPubKey},
        blockdata::{
            locktime::absolute::LockTime,
            opcodes, script,
            script::PushBytes,
            transaction::{OutPoint, Transaction},
        },
        hashes::{sha256::Hash as Sha256, sha256d::Hash as Sha256d, Hash},
        secp256k1::{
            rand::thread_rng, KeyPair, Message, Parity, PublicKey, Secp256k1, SecretKey,
            XOnlyPublicKey,
        },
        sighash::{Prevouts, SighashCache, TapSighashType},
        taproot::{ControlBlock, LeafVersion, TapLeafHash, TaprootBuilder, TaprootSpendInfo},
        Address, Network, Script, ScriptBuf, Sequence, TxIn, TxOut, Txid, Witness,
    },
    blockchain::{ConfigurableBlockchain, RpcBlockchain, RpcConfig},
    database::SqliteDatabase,
    descriptor::Descriptor,
    wallet::{AddressIndex, AddressInfo},
    Balance,
    KeychainKind::{self, External, Internal},
    SyncOptions, Wallet,
};
use bitcoin::consensus::Encodable;
use bitcoin::key::TweakedPublicKey;
use hex::ToHex;
use reqwest;
use reqwest::header::CONTENT_TYPE;
use std::mem;

use std::sync::Mutex;

use bytes::BufMut;
use http::status::StatusCode;
use std::str::FromStr;

use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

use crate::client::{self, LooperClient};
use crate::settings;
use looper::lnd::client::LNDGateway;
use looper::services::loop_out::{LoopOutRequest, LoopOutResponse, TaprootScriptInfo};
use looper::wallet::LooperWallet;

pub struct ClientConfig {
    pub looper_url: String,
}

pub struct LoopOutService {
    // cfg: &client::Config,
    // client: LooperClient,
    wallet: Mutex<LooperWallet>,
}

const FEE_LIMIT: i64 = 300;
// TODO: find a better home
const DEFAULT_CODESEP: u32 = 0xffff_ffff;

impl LoopOutService {
    pub fn new(wallet: LooperWallet) -> Self {
        // let client = LooperClient::new(cfg);

        Self {
            // cfg,
            // client,
            wallet: Mutex::new(wallet),
        }
    }

    // pub fn new_loop_out(&self, amount: i64, address: Address) -> SendResponse {
    //     // lock wallet, get necessary info
    //     let wallet = self.wallet.lock().unwrap();
    //     let (pubkey, pubkey_idx) = wallet.new_pubkey().unwrap();
    //     let keypair = wallet.get_keypair(pubkey_idx).unwrap();
    //     let curr_height = wallet.get_height().unwrap();
    //     mem::drop(wallet);
    //     // unlock wallet

    //     // TODO: hit API
    //     // let resp = self.new_loop_out_request(pubkey, amount);

    //     // TODO: verify that taproot external key is correct, internal key unspendable, and tree is correct

    //     // handle_loop_out_response(resp);
    // }

    pub fn refresh_wallet(&self) -> u32 {
        let wallet = self.wallet.lock().unwrap();
        (*wallet).sync().unwrap();
        let curr_height = (*wallet).get_height().unwrap();
        mem::drop(wallet);
        curr_height
    }

    pub fn handle_loop_out_response(
        &self,
        resp: LoopOutResponse,
        pubkey_idx: u32,
        address: Address,
        amount: u64,
        curr_height: u32,
    ) {
        let keypair = self.wallet.lock().unwrap().get_keypair(pubkey_idx);
        let (buyer_pubkey, _) = XOnlyPublicKey::from_keypair(&keypair);

        let pay_result = LNDGateway::pay_invoice_sync(resp.invoice, FEE_LIMIT).unwrap();
        log::info!("preimage: {}", hex::encode(&pay_result.payment_preimage));

        let int_pubkey_bytes = hex::decode(&resp.taproot_script_info.internal_key).unwrap();
        let internal_key =
            bitcoin::key::XOnlyPublicKey::from_slice(&int_pubkey_bytes as &[u8]).unwrap();

        let ext_pubkey_bytes = hex::decode(&resp.taproot_script_info.external_key).unwrap();
        let external_key =
            bitcoin::key::XOnlyPublicKey::from_slice(&ext_pubkey_bytes as &[u8]).unwrap();
        let external_key = TweakedPublicKey::dangerous_assume_tweaked(external_key);
        let prev_scriptpubkey = ScriptBuf::new_v1_p2tr_tweaked(external_key);

        let prev_txout: TxOut = TxOut {
            value: amount,
            script_pubkey: prev_scriptpubkey,
        };

        // TODO: claim UTXO
        let txid_bytes = hex::decode(resp.txid).unwrap();

        let txid = Hash::from_slice(&txid_bytes).unwrap();

        let mut inputs = vec![TxIn {
            previous_output: OutPoint {
                txid: Txid::from_raw_hash(txid),
                vout: resp.vout,
            },
            sequence: Sequence(0),
            // to be filled later
            witness: Witness::new(),
            script_sig: ScriptBuf::new(),
        }];

        let outputs = vec![TxOut {
            // TODO: calculate fee and subtract from amount. for now we just pay 1000 sats fee
            value: amount - 1_000,
            script_pubkey: address.script_pubkey(),
        }];

        // stop fee-sniping by setting current height to locktime
        let locktime = LockTime::from_height(curr_height).expect("invalid locktime");
        let mut tx = Transaction {
            version: 2,
            lock_time: locktime,
            input: inputs,
            output: outputs,
        };

        let looper_pubkey = XOnlyPublicKey::from_str(&resp.looper_pubkey).unwrap();

        // we could look at the resp.taproot_script_info, but this is easier
        let mut payment_hash = [0u8; 32];
        hex::decode_to_slice(&resp.loop_info.loop_hash, &mut payment_hash as &mut [u8]).unwrap();

        // TODO: somewhat redundant, make wallet more composable
        let locktime = LockTime::from_height(resp.loop_info.cltv_expiry).expect("invalid height");
        let timeout_script = LooperWallet::new_timeout_script(looper_pubkey, locktime);
        let htlc_leaf = LooperWallet::new_htlc_script(&buyer_pubkey, &payment_hash);
        let tr = LooperWallet::build_taproot(&htlc_leaf, &timeout_script, internal_key);

        let tapleaf_hash = TapLeafHash::from_script(htlc_leaf.as_script(), LeafVersion::TapScript);
        // TODO: calculate fee here, then adjust output amount

        let mut sighash_cache = SighashCache::new(&tx);
        let sighash = sighash_cache
            .taproot_script_spend_signature_hash(
                0,
                &Prevouts::All(&[&prev_txout][..]),
                tapleaf_hash,
                TapSighashType::Default,
            )
            .unwrap()
            .to_byte_array();

        let secp256k1 = Secp256k1::new();

        let msg = Message::from_slice(&sighash[..]).unwrap();
        let sig = secp256k1.sign_schnorr(&msg, &keypair);

        let control_block = match tr.control_block(&(htlc_leaf.clone(), LeafVersion::TapScript)) {
            Some(control_block) => control_block,
            None => {
                panic!("control block is none");
            }
        };
        // TODO: possible to verify this using verify_taproot_commitment

        // TODO: check that sighash type was default, if not must add a byte
        let control_block_bytes = control_block.serialize();
        let witness = Witness::from_vec(vec![
            pay_result.payment_preimage.to_vec(),
            sig.as_ref().to_vec(),
            htlc_leaf.into_bytes(),
            control_block_bytes,
        ]);

        tx.input[0].witness = witness;

        let mut buf = vec![].writer();
        tx.consensus_encode(&mut buf).unwrap();

        log::info!("tx: {}", hex::encode(buf.into_inner()));

        // all wrong let witness = vec![
        //     sig.serialize_der().to_vec(),
        //     pubkey.serialize().to_vec(),
        //     vec![].to_vec(),
        //     htlc_leaf.as_script().to_bytes(),
        // ];

        self.wallet.lock().unwrap().broadcast_tx(&tx).unwrap();
    }

    // pub fn new_loop_out_request(&self, pubkey: String, amount: i64) -> LoopOutResponse {
    //     let req = LoopOutRequest {
    //         pubkey: pubkey,
    //         amount: amount,
    //     };

    //     let url = format!("{}/loop_out", self.cfg.looper_url);
    //     let res = self.client.get(&url).send().await.unwrap();

    //     // parse & handle. how to do async without making everything async?
    // }
}
