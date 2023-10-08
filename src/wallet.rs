use std::env;
use std::path::Path;
use std::str::FromStr;
use std::thread;

use bitcoin::{address, Address, Txid};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, Mutex,
};

use crate::services::errors::{LooperError, LooperErrorResponse};
use http::StatusCode;

use crate::settings;
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
            rand::thread_rng, KeyPair, Parity, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey,
        },
        taproot,
        taproot::{TaprootBuilder, TaprootSpendInfo},
        Network, ScriptBuf,
    },
    blockchain::{ConfigurableBlockchain, GetHeight, GetTx, RpcBlockchain, RpcConfig, WalletSync},
    database::SqliteDatabase,
    descriptor::Descriptor,
    wallet::{wallet_name_from_descriptor, AddressIndex, AddressInfo},
    Balance,
    KeychainKind::{self, External, Internal},
    SignOptions, SyncOptions, Wallet,
};
use bdk::{blockchain::Blockchain, sled};
use config::{Config, Environment};

pub struct LooperWallet {
    blockchain: RpcBlockchain,
    xprv: ExtendedPrivKey,
    index: std::sync::Mutex<u32>,
    wallet: Wallet<sled::Tree>,
}

impl LooperWallet {
    pub fn new(cfg: &settings::Config) -> Self {
        let secp256k1 = Secp256k1::new();
        let xprv_str = env::var("LOOPER_XPRV").unwrap();
        let xprv = ExtendedPrivKey::from_str(&xprv_str).unwrap();
        // let xpub = ExtendedPubKey::from_priv(&secp256k1, &xprv);

        let wallet_xprv = xprv
            .ckd_priv(&secp256k1, ChildNumber::Normal { index: 2 })
            .unwrap();
        let ext_descriptor = format!("wpkh({}/0/*)", wallet_xprv);
        let int_descriptor = format!("wpkh({}/1/*)", wallet_xprv);
        let network =
            Network::from_str(cfg.get_string("bitcoin.network").unwrap().as_str()).unwrap();

        let wallet_name = wallet_name_from_descriptor(
            &ext_descriptor,
            Some(&int_descriptor),
            network,
            &secp256k1,
        )
        .unwrap();
        let wallet_db = LooperWallet::new_sled_db(wallet_name.clone());
        let wallet =
            Wallet::new(&ext_descriptor, Some(&int_descriptor), network, wallet_db).unwrap();

        let blockchain = LooperWallet::build_rpc_blockchain(cfg, wallet_name);

        let s = Self {
            blockchain,
            xprv,
            index: std::sync::Mutex::new(0),
            wallet,
        };

        s.sync().unwrap();

        s
    }

    fn new_sled_db(wallet_name: String) -> sled::Tree {
        let datadir = Path::new(".looper");
        let database = sled::open(datadir).unwrap();
        let db_tree: sled::Tree = database.open_tree(wallet_name.clone()).unwrap();

        db_tree
    }

    pub fn new_address(&self) -> AddressInfo {
        self.wallet.get_address(AddressIndex::LastUnused).unwrap()
    }

    pub fn validate_address(
        &self,
        address: &str,
    ) -> Result<address::Address<address::NetworkChecked>, LooperErrorResponse> {
        let addr = address::Address::from_str(address).unwrap();
        let addr = addr.require_network(self.wallet.network()).map_err(|e| {
            LooperErrorResponse::new(
                StatusCode::BAD_REQUEST,
                format!("invalid address: {}", address),
                "".to_string(),
            )
        })?;

        Ok(addr)
    }

    pub fn sync(&self) -> Result<(), bdk::Error> {
        self.wallet.sync(&self.blockchain, SyncOptions::default())
    }

    pub fn get_network(&self) -> Network {
        self.wallet.network()
    }

    pub fn get_balance(&self) -> Balance {
        self.wallet.get_balance().unwrap()
    }

    pub fn send_to_address(&self, address: &str, amount: u64) -> Result<Txid, LooperErrorResponse> {
        let mut tx_builder = self.wallet.build_tx();
        let addr = self.validate_address(address)?;

        let curr_height = self.get_height().unwrap();
        let locktime = LockTime::from_height(curr_height).expect("invalid height");
        tx_builder
            // TODO: bad for privacy, but simplest way to ensure we know the vout. Also, this amount will usually be round
            // and the change will be to wpkh for now, so overall poor privacy anyway. All fixable.
            .ordering(bdk::wallet::tx_builder::TxOrdering::Untouched)
            .add_recipient(addr.script_pubkey(), amount)
            .fee_rate(bdk::FeeRate::from_sat_per_vb(5.0))
            .nlocktime(locktime);

        let (mut psbt, _details) = tx_builder.finish().map_err(|e| {
            LooperErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to build tx: {:?}", e),
                "".to_string(),
            )
        })?;

        let finalized = self
            .wallet
            .sign(&mut psbt, SignOptions::default())
            .map_err(|e| {
                LooperErrorResponse::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to sign tx: {:?}", e),
                    "".to_string(),
                )
            })?;

        if !finalized {
            return Err(LooperErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "failed to finalize tx".to_string(),
                "".to_string(),
            ));
        }

        let tx = psbt.extract_tx();
        let txid = tx.txid();
        self.broadcast_tx(&tx)?;

        Ok(txid)
    }

    pub fn broadcast_tx(&self, tx: &Transaction) -> Result<(), LooperErrorResponse> {
        self.blockchain.broadcast(tx).map_err(|e| {
            LooperErrorResponse::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to broadcast tx: {:?}", e),
                "".to_string(),
            )
        })?;

        Ok(())
    }

    // TODO: make priv
    // Maybe force pubkey even and return that instead of xonly
    pub fn new_pubkey(&self) -> (XOnlyPublicKey, u32) {
        let secp256k1 = Secp256k1::new();
        // TODO: use tokio and async lock
        let mut pk_index = self.index.lock().unwrap();
        let index = *pk_index;
        let child_num = vec![ChildNumber::Normal { index }];
        let xsk: ExtendedPrivKey = self.xprv.derive_priv(&secp256k1, &child_num).unwrap();
        *pk_index += 1;

        let keypair = xsk.to_keypair(&secp256k1);
        let (pk, _) = keypair.x_only_public_key();

        (pk, index)
    }

    pub fn get_keypair(&self, index: u32) -> KeyPair {
        let secp256k1 = Secp256k1::new();
        let child_num = vec![ChildNumber::Normal { index }];
        let xsk: ExtendedPrivKey = self.xprv.derive_priv(&secp256k1, &child_num).unwrap();

        xsk.to_keypair(&secp256k1)
    }

    fn build_rpc_blockchain(cfg: &Config, wallet_name: String) -> RpcBlockchain {
        let network =
            Network::from_str(cfg.get_string("bitcoin.network").unwrap().as_str()).unwrap();
        let url = cfg.get_string("bitcoin.url").unwrap();
        let username = cfg.get_string("bitcoin.user").unwrap();
        let password = cfg.get_string("bitcoin.pass").unwrap();

        let rpc_config = RpcConfig {
            url: url,
            auth: bdk::blockchain::rpc::Auth::UserPass {
                username: username,
                password: password,
            },
            network: network,
            wallet_name: wallet_name,
            sync_params: None,
        };
        let blockchain = RpcBlockchain::from_config(&rpc_config).unwrap();
        return blockchain;
    }

    pub fn new_htlc_script(claimant_pk: &XOnlyPublicKey, payment_hash: &[u8; 32]) -> ScriptBuf {
        // miniscript: "and(pk({claimant_pk}),sha256({payment_hash}))"
        // Script: OP_PUSH32 <32 claimant_pk> OP_CHECKSIGVERIFY OP_SIZE 32 OP_EQUALVERIFY OP_SHA256 OP_PUSH32 <32 payment_hash> OP_EQUAL
        let htlc_script = script::Builder::new()
            .push_x_only_key(claimant_pk)
            .push_opcode(opcodes::all::OP_CHECKSIGVERIFY)
            .push_opcode(opcodes::all::OP_SIZE)
            .push_int(32)
            .push_opcode(opcodes::all::OP_EQUALVERIFY)
            .push_opcode(opcodes::all::OP_SHA256)
            .push_slice(payment_hash)
            .push_opcode(opcodes::all::OP_EQUAL);

        htlc_script.into_script()
    }

    pub fn new_timeout_script(claimee_pk: XOnlyPublicKey, timeout: LockTime) -> ScriptBuf {
        // miniscript: "and(pk({claimee_pk}),after({timeout}))"
        // Script: <CLTV> OP_CLTV OP_DROP <claimee_pk> OP_CHECKSIG
        let timeout_script = script::Builder::new()
            .push_lock_time(timeout)
            .push_opcode(opcodes::all::OP_CLTV)
            .push_opcode(opcodes::all::OP_DROP)
            .push_x_only_key(&claimee_pk)
            .push_opcode(opcodes::all::OP_CHECKSIG);

        timeout_script.into_script()
    }

    pub fn new_htlc(
        claimant_pk: XOnlyPublicKey,
        claimee_pk: XOnlyPublicKey,
        payment_hash: &[u8; 32],
        cltv: u32,
    ) -> (TaprootSpendInfo, SecretKey) {
        let htlc_script = LooperWallet::new_htlc_script(&claimant_pk, payment_hash);

        let locktime = LockTime::from_height(cltv).expect("invalid height");

        let timeout_script = LooperWallet::new_timeout_script(claimee_pk, locktime);

        let (internal_tapkey, internal_tapseckey) = LooperWallet::new_unspendable_internal_key();
        let tr = LooperWallet::build_taproot(&htlc_script, &timeout_script, internal_tapkey);

        (tr, internal_tapseckey)
    }

    pub fn build_taproot(
        htlc_script: &ScriptBuf,
        timeout_script: &ScriptBuf,
        internal_tapkey: XOnlyPublicKey,
    ) -> TaprootSpendInfo {
        let secp256k1 = Secp256k1::new();
        let tr = TaprootBuilder::new()
            .add_leaf(1, htlc_script.clone())
            .unwrap()
            .add_leaf(1, timeout_script.clone())
            .unwrap()
            .finalize(&secp256k1, internal_tapkey)
            .unwrap();

        tr
    }

    fn new_unspendable_internal_key() -> (XOnlyPublicKey, SecretKey) {
        let pk_h = PublicKey::from_str(
            "0250929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
        )
        .unwrap();
        let secp256k1 = Secp256k1::new();
        let mut rng = thread_rng();
        let (r, _) = secp256k1.generate_keypair(&mut rng);
        let pk_r = PublicKey::from_secret_key(&secp256k1, &r);
        let p: XOnlyPublicKey = pk_r.combine(&pk_h).unwrap().into();

        (p, r)
    }

    pub fn get_height(&self) -> Result<u32, bdk::Error> {
        self.blockchain.get_height()
    }
}
