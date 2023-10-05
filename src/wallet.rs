use std::env;
use std::str::FromStr;
use std::thread;

use tokio::sync::{
    mpsc::{Receiver, Sender},
    oneshot, Mutex,
};

use bdk::{
    bitcoin::{
        hashes::{sha256::Hash as Sha256, sha256d::Hash as Sha256d, Hash},
        secp256k1::{rand::thread_rng, Secp256k1, PublicKey, SecretKey},
        util::{
            bip32::{ExtendedPrivKey, ExtendedPubKey},
            key::{ PrivateKey, KeyPair, XOnlyPublicKey},
            taproot,
            taproot::{TaprootBuilder, TaprootSpendInfo},
        },
        Network,
        blockdata::{script, opcodes}, schnorr::{UntweakedKeyPair, UntweakedPublicKey},
    },
    database::SqliteDatabase,
    blockchain::{ConfigurableBlockchain, RpcBlockchain, RpcConfig},
    wallet::{wallet_name_from_descriptor, AddressIndex},
    Balance, SyncOptions, Wallet,
};
use crate::settings;
use config::{Config, Environment};

macro_rules! wallet_cmd {
    ($wallet:expr, $cmd:expr, $($matcher:pat => $result:expr),*) => {
        let (tx, rx) = oneshot::channel();
        $wallet.tx.blocking_send(($cmd, tx)).unwrap();
        match rx.blocking_recv().unwrap() {
            $($matcher => $result,)*
            WalletResult::Error { error } => Err(error),
            _ => panic!(),
        }
    };
}

pub struct LooperWallet {
    blockchain: RpcBlockchain,
    wallet: Wallet<SqliteDatabase>,
    rx: Mutex<Receiver<(WalletCmd, oneshot::Sender<WalletResult>)>>,
    tx: Sender<(WalletCmd, oneshot::Sender<WalletResult>)>,
}

impl LooperWallet {
    pub fn new() -> Self {
        let cfg = settings::build_config().expect("failed to build wallet config");
        let secp256k1 = Secp256k1::new();
        let xprv_str = env::var("LOOPER_XPRV").unwrap();
        let xprv = ExtendedPrivKey::from_str(&xprv_str).unwrap();
        
        let xpub = ExtendedPubKey::from_priv(&secp256k1, &xprv);
        let ext_descriptor = format!("{}/0/*", xpub);
        let int_descriptor = format!("{}/1/*", xpub);
        let network = Network::from_str(cfg.get_string("bitcoin.network").unwrap().as_str()).unwrap();
        let wallet = Wallet::new(
            &ext_descriptor, 
            Some(&int_descriptor), 
            network, 
            SqliteDatabase::new(cfg.get_string("bitcoin.dbpath").unwrap().as_str())).unwrap();
        let wallet_name =
            wallet_name_from_descriptor(&ext_descriptor,  Some(&int_descriptor), network, &secp256k1).unwrap();
       
        
        let blockchain = LooperWallet::build_rpc_blockchain(&cfg, wallet_name);
        return Self{
            blockchain: blockchain,
            wallet: wallet,
        }
    }


    pub fn start(&self) {
        thread::Builder::new()
            .name("wallet".to_string())
            .spawn(move || {
                log::info!("starting wallet thread");
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();

                runtime.block_on(async move {
                    loop {
                        let (cmd, res_tx) = self.rx.lock().await.recv().await.unwrap();
                        self.handle_cmd(cmd, res_tx).await;
                    }
                })
            })
            .unwrap();
    }

    async fn handle_cmd(&self, cmd: WalletCmd, res_tx: oneshot::Sender<WalletResult>) {
        let res = match cmd {
            LooperWallet::NewHTLC{
                claimant_pk,
            } => {
                log::info!("new htlc");
                let (tapkey, tapseckey) = LooperWallet::new_htlc(claimant_pk, self.wallet.get_address(address_index))
            }
        }
    }


    fn build_rpc_blockchain(cfg: &Config, wallet_name: String) -> RpcBlockchain {
        let network = Network::from_str(cfg.get_string("bitcoin.network").unwrap().as_str()).unwrap();
        let url = cfg.get_string("bitcoin.url").unwrap();
        let username = cfg.get_string("bitcoin.user").unwrap();
        let password = cfg.get_string("bitcoin.pass").unwrap();
        
        let rpc_config = RpcConfig {
            url: url,
            auth: bdk::blockchain::rpc::Auth::UserPass{
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

    fn new_htlc_script(claimant_pk: PublicKey, payment_hash: &[u8]) -> script::Script {
        let xonly_pk: XOnlyPublicKey = claimant_pk.into(); 
        // miniscript: "and(pk({claimant_pk}),sha256({payment_hash}))"
        // Script: <claimant_pk> OP_CHECKSIGVERIFY OP_SHA256 <payment_hash> OP_EQUAL
        let htlc_script = script::Builder::new()
            .push_x_only_key(&xonly_pk)
            .push_opcode(opcodes::all::OP_CHECKSIGVERIFY)
            .push_opcode(opcodes::all::OP_SIZE)
            .push_int(32)
            .push_opcode(opcodes::all::OP_EQUALVERIFY)
            .push_opcode(opcodes::all::OP_SHA256)
            .push_slice(&payment_hash)
            .push_opcode(opcodes::all::OP_EQUAL);
        return htlc_script.into_script();
    }

    fn new_timeout_script(claimee_pk: PublicKey, timeout: i64) -> script::Script {
        let xonly_pk: XOnlyPublicKey = claimee_pk.into(); 
        // miniscript: "and(pk({claimee_pk}),after({timeout}))"
        // Script: <CLTV> OP_CLTV OP_DROP <claimee_pk> OP_CHECKSIG
        let timeout_script = script::Builder::new()
            .push_int(timeout)
            .push_opcode(opcodes::all::OP_CLTV)
            .push_opcode(opcodes::all::OP_DROP)
            .push_x_only_key(&xonly_pk)
            .push_opcode(opcodes::all::OP_CHECKSIG);
        return timeout_script.into_script();
    }

    pub fn new_htlc(claimant_pk: PublicKey, claimee_pk: PublicKey, payment_hash: &[u8], timeout: i64) -> (TaprootSpendInfo, SecretKey) {
        let secp256k1 = Secp256k1::new();
        let htlc_script = LooperWallet::new_htlc_script(claimant_pk, payment_hash);
        let timeout_script = LooperWallet::new_timeout_script(claimee_pk, timeout);

        let (internal_tapkey, internal_tapseckey) = LooperWallet::new_unspendable_internal_key();
        let tr = TaprootBuilder::new()
            .add_leaf(
                1,
                htlc_script,
            ).unwrap()
            .add_leaf(
                1,
                timeout_script,
            ).unwrap()
            .finalize(&secp256k1, internal_tapkey).unwrap();

        return (tr, internal_tapseckey)
    }

    fn new_unspendable_internal_key() -> (UntweakedPublicKey, SecretKey) {
        let pk_h = PublicKey::from_str("0250929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap();
        let secp256k1 = Secp256k1::new();
        let mut rng = thread_rng();
        let (r, _) = secp256k1.generate_keypair(&mut rng);
        let pk_r = PublicKey::from_secret_key(&secp256k1, &r);
        let p: XOnlyPublicKey = pk_r.combine(&pk_h).unwrap().into();
        return (p, r);
    }
}