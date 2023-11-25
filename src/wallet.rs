use std::path::Path;
use std::str::FromStr;
// use std::thread;

use bitcoin::address;

// use crate::services::errors::{LooperError, LooperErrorResponse};

use crate::{mempool, settings};
use bdk::{
    bitcoin::{
        bip32::{ChildNumber, ExtendedPrivKey},
        blockdata::{locktime::absolute::LockTime, opcodes, script, transaction::Transaction},
        // hashes::{sha256::Hash as Sha256, sha256d::Hash as Sha256d, Hash},
        secp256k1::{rand::thread_rng, KeyPair, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey},
        // taproot,
        taproot::{TaprootBuilder, TaprootSpendInfo},
        Network,
        ScriptBuf,
    },
    blockchain::{ConfigurableBlockchain, GetHeight, RpcBlockchain, RpcConfig},
    // database::SqliteDatabase,
    // descriptor::Descriptor,
    wallet::{wallet_name_from_descriptor, AddressIndex, AddressInfo},
    Balance,
    FeeRate,
    SignOptions,
    SyncOptions,
    Wallet,
};
use bdk::{blockchain::Blockchain, sled};
use config::Config;
use std::sync::Mutex;

pub struct LooperWallet {
    blockchain: RpcBlockchain,
    xprv: ExtendedPrivKey,
    index: Mutex<u32>,
    wallet: Wallet<sled::Tree>,
}

impl LooperWallet {
    // TODO: use WalletCfg instead of Config
    pub fn new(cfg: &settings::Config) -> Result<Self, WalletError> {
        let secp256k1 = Secp256k1::new();
        // let xprv_str = env::var("LOOPER_XPRV")
        //     .map_err(|e| WalletError::new(format!("LOOPER_XPRV unset: {:?}", e.to_string())))?;
        let xprv_str = "tprv8ZgxMBicQKsPd7Uf69XL1XwhmjHopUGep8GuEiJDZmbQz6o58LninorQAfcKZWARbtRtfnLcJ5MQ2AtHcQJCCRUcMRvmDUjyEmNUWwx8UbK".to_string();
        let xprv = ExtendedPrivKey::from_str(&xprv_str)
            .map_err(|e| WalletError::new(format!("failed to parse xprv: {:?}", e.to_string())))?;
        // let xpub = ExtendedPubKey::from_priv(&secp256k1, &xprv);

        let wallet_xprv = xprv
            .ckd_priv(&secp256k1, ChildNumber::Normal { index: 2 })
            .map_err(|e| {
                WalletError::new(format!("failed to derive wallet xprv: {:?}", e.to_string()))
            })?;
        let ext_descriptor = format!("wpkh({}/0/*)", wallet_xprv);
        let int_descriptor = format!("wpkh({}/1/*)", wallet_xprv);
        let network = Self::parse_network_from_config(cfg)?;

        let wallet_name = wallet_name_from_descriptor(
            &ext_descriptor,
            Some(&int_descriptor),
            network,
            &secp256k1,
        )
        .map_err(|e| {
            WalletError::new(format!(
                "failed to get wallet name from descriptor: {:?}",
                e.to_string()
            ))
        })?;
        let wallet_db = LooperWallet::new_sled_db(wallet_name.clone())?;
        let wallet = Wallet::new(&ext_descriptor, Some(&int_descriptor), network, wallet_db)
            .map_err(|e| {
                WalletError::new(format!("failed to create wallet: {:?}", e.to_string()))
            })?;

        let blockchain = LooperWallet::build_rpc_blockchain(cfg, wallet_name)?;

        let looper_wallet = Self {
            blockchain,
            xprv,
            index: Mutex::new(0),
            wallet,
        };

        looper_wallet.sync()?;

        Ok(looper_wallet)
    }

    fn new_sled_db(wallet_name: String) -> Result<sled::Tree, WalletError> {
        let datadir = Path::new(".looper");
        let database = sled::open(datadir).map_err(|e| {
            WalletError::new(format!("failed to open sled db: {:?}", e.to_string()))
        })?;
        let db_tree: sled::Tree = database.open_tree(wallet_name.clone()).map_err(|e| {
            WalletError::new(format!("failed to open sled db tree: {:?}", e.to_string()))
        })?;

        Ok(db_tree)
    }

    pub fn new_address(&self) -> Result<AddressInfo, WalletError> {
        self.wallet
            .get_address(AddressIndex::LastUnused)
            .map_err(|e| {
                WalletError::new(format!("failed to get new address: {:?}", e.to_string()))
            })
    }

    pub fn validate_address(
        &self,
        address: &str,
    ) -> Result<address::Address<address::NetworkChecked>, WalletError> {
        let addr = address::Address::from_str(address).map_err(|e| {
            WalletError::new(format!("failed to parse address: {:?}", e.to_string()))
        })?;
        let addr = addr.require_network(self.wallet.network()).map_err(|e| {
            WalletError::new(format!("invalid address: {} {:?}", address, e.to_string()))
        })?;

        Ok(addr)
    }

    pub fn sync(&self) -> Result<(), WalletError> {
        // TODO: maybe load current index from db here too.
        self.wallet
            .sync(&self.blockchain, SyncOptions::default())
            .map_err(|e| WalletError::new(format!("failed to sync wallet: {:?}", e.to_string())))
    }

    pub fn get_network(&self) -> Network {
        self.wallet.network()
    }

    fn parse_network_from_config(cfg: &Config) -> Result<Network, WalletError> {
        let network = Network::from_str(&cfg.get_string("bitcoin.network").map_err(|e| {
            WalletError::new(format!(
                "failed to get bitcoin network from config: {:?}",
                e.to_string()
            ))
        })?)
        .map_err(|e| {
            WalletError::new(format!(
                "failed to get bitcoin network from config: {:?}",
                e.to_string()
            ))
        })?;

        Ok(network)
    }

    pub fn get_balance(&self) -> Result<Balance, WalletError> {
        self.wallet.get_balance().map_err(|e| {
            WalletError::new(format!("failed to get wallet balance: {:?}", e.to_string()))
        })
    }

    pub async fn get_mempool_fee_rate() -> Result<FeeRate, WalletError> {
        let fee_rate = mempool::get_mempool_fee_rate(
            // TODO: make this configurable
            mempool::MempoolFeePriority::Blocks6,
        )
        .await
        .map_err(|e| WalletError::new(format!("failed to get mempool fee estimate: {:?}", e)))?;

        Ok(fee_rate)
    }

    pub fn estimate_fee_rate(&self, target: usize) -> Result<FeeRate, WalletError> {
        self.blockchain.estimate_fee(target).map_err(|e| {
            WalletError::new(format!("failed to estimate fee rate: {:?}", e.to_string()))
        })
    }

    pub fn send_to_address(
        &self,
        address: &str,
        amount: u64,
        fee_rate: &FeeRate,
    ) -> Result<Transaction, WalletError> {
        let mut tx_builder = self.wallet.build_tx();
        let addr = self.validate_address(address)?;

        let curr_height = self.get_height().map_err(|e| {
            WalletError::new(format!("failed to get current height: {:?}", e.to_string()))
        })?;
        let locktime = LockTime::from_height(curr_height).map_err(|e| {
            WalletError::new(format!("failed to get locktime: {:?}", e.to_string()))
        })?;
        tx_builder
            // TODO: bad for privacy, but simplest way to ensure we know the vout. Also, this amount will usually be round
            // and the change will be to wpkh for now, so overall poor privacy anyway. All fixable.
            .ordering(bdk::wallet::tx_builder::TxOrdering::Untouched)
            .add_recipient(addr.script_pubkey(), amount)
            .fee_rate(*fee_rate)
            .nlocktime(locktime);

        let (mut psbt, _details) = tx_builder
            .finish()
            .map_err(|e| WalletError::new(format!("failed to build tx: {:?}", e)))?;

        let finalized = self
            .wallet
            .sign(&mut psbt, SignOptions::default())
            .map_err(|e| WalletError::new(format!("failed to sign tx: {:?}", e)))?;

        if !finalized {
            return Err(WalletError::new("failed to finalize tx".to_string()));
        }

        let tx = psbt.extract_tx();

        Ok(tx)
    }

    pub fn broadcast_tx(&self, tx: &Transaction) -> Result<(), WalletError> {
        self.blockchain
            .broadcast(tx)
            .map_err(|e| WalletError::new(format!("failed to broadcast tx: {:?}", e)))?;

        Ok(())
    }

    // TODO: make priv
    // Maybe force pubkey even and return that instead of xonly
    pub fn new_pubkey(&self) -> Result<(XOnlyPublicKey, u32), WalletError> {
        let secp256k1 = Secp256k1::new();
        // TODO: use tokio and async lock
        let mut pk_index = self.index.lock().map_err(|e| {
            WalletError::new(format!("failed to get lock on index: {:?}", e.to_string()))
        })?;
        let index = *pk_index;
        let child_num = vec![ChildNumber::Normal { index }];
        let xsk: ExtendedPrivKey = self.xprv.derive_priv(&secp256k1, &child_num).map_err(|e| {
            WalletError::new(format!("failed to derive priv key: {:?}", e.to_string()))
        })?;
        *pk_index += 1;

        let keypair = xsk.to_keypair(&secp256k1);
        let (pk, _) = keypair.x_only_public_key();

        Ok((pk, index))
    }

    pub fn get_keypair(&self, index: u32) -> Result<KeyPair, WalletError> {
        let secp256k1 = Secp256k1::new();
        let child_num = vec![ChildNumber::Normal { index }];
        let xsk: ExtendedPrivKey = self.xprv.derive_priv(&secp256k1, &child_num).map_err(|e| {
            WalletError::new(format!("failed to derive priv key: {:?}", e.to_string()))
        })?;

        Ok(xsk.to_keypair(&secp256k1))
    }

    fn build_rpc_blockchain(
        cfg: &Config,
        wallet_name: String,
    ) -> Result<RpcBlockchain, WalletError> {
        // TODO break these out, DRY it up
        let network = Self::parse_network_from_config(cfg)?;
        let url = cfg.get_string("bitcoin.url").map_err(|e| {
            WalletError::new(format!(
                "failed to get bitcoin url from config: {:?}",
                e.to_string()
            ))
        })?;
        let username = cfg.get_string("bitcoin.user").map_err(|e| {
            WalletError::new(format!(
                "failed to get bitcoin username from config: {:?}",
                e.to_string()
            ))
        })?;
        let password = cfg.get_string("bitcoin.pass").map_err(|e| {
            WalletError::new(format!(
                "failed to get bitcoin password from config: {:?}",
                e.to_string()
            ))
        })?;

        let rpc_config = RpcConfig {
            url,
            auth: bdk::blockchain::rpc::Auth::UserPass { username, password },
            network,
            wallet_name,
            sync_params: None,
        };
        let blockchain = RpcBlockchain::from_config(&rpc_config).map_err(|e| {
            WalletError::new(format!(
                "failed to create rpc blockchain: {:?}",
                e.to_string()
            ))
        })?;

        Ok(blockchain)
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
    ) -> Result<(TaprootSpendInfo, SecretKey), WalletError> {
        let htlc_script = LooperWallet::new_htlc_script(&claimant_pk, payment_hash);

        let locktime = LockTime::from_height(cltv).expect("invalid height");

        let timeout_script = LooperWallet::new_timeout_script(claimee_pk, locktime);

        let (internal_tapkey, internal_tapseckey) = LooperWallet::new_unspendable_internal_key()?;
        let tr = LooperWallet::build_taproot(&htlc_script, &timeout_script, internal_tapkey)?;

        Ok((tr, internal_tapseckey))
    }

    pub fn build_taproot(
        htlc_script: &ScriptBuf,
        timeout_script: &ScriptBuf,
        internal_tapkey: XOnlyPublicKey,
    ) -> Result<TaprootSpendInfo, WalletError> {
        let secp256k1 = Secp256k1::new();

        TaprootBuilder::new()
            .add_leaf(1, htlc_script.clone())
            .map_err(|e| WalletError::new(format!("failed to add leaf: {:?}", e.to_string())))?
            .add_leaf(1, timeout_script.clone())
            .map_err(|e| WalletError::new(format!("failed to add leaf: {:?}", e.to_string())))?
            .finalize(&secp256k1, internal_tapkey)
            .map_err(|_tb| WalletError::new("failed to finalize taproot".to_string()))
    }

    fn new_unspendable_internal_key() -> Result<(XOnlyPublicKey, SecretKey), WalletError> {
        // FROM BIP342
        let pk_h = PublicKey::from_str(
            "0250929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0",
        )
        .unwrap();
        let secp256k1 = Secp256k1::new();
        let mut rng = thread_rng();
        let (r, _) = secp256k1.generate_keypair(&mut rng);
        let pk_r = PublicKey::from_secret_key(&secp256k1, &r);
        let p: XOnlyPublicKey = pk_r
            .combine(&pk_h)
            .map_err(|e| WalletError::new(format!("failed to combine keys: {:?}", e.to_string())))?
            .into();

        Ok((p, r))
    }

    pub fn get_height(&self) -> Result<u32, bdk::Error> {
        self.blockchain.get_height()
    }
}

#[derive(Debug)]
pub struct WalletError {
    pub message: String,
}

impl WalletError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
