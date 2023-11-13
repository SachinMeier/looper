use bdk::bitcoin::{
    secp256k1::{self, Secp256k1, XOnlyPublicKey},
    taproot::TaprootSpendInfo,
    Address, Network,
};
use std::mem;

// use diesel_async::{pg::AsyncPgConnection, AsyncConnection};

use std::str::FromStr;
use tokio::sync::Mutex;

use crate::{
    db::{self, DB},
    lnd::client::LNDGateway,
    mempool,
    models::{self, FullLoopOutData, Invoice, NewInvoice, NewScript, NewUTXO, Script, Utxo},
    settings,
    wallet::LooperWallet,
};

// TODO: maybe make configurable
#[allow(dead_code)]
pub const TARGET_CONFS: usize = 6;

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
    ) -> Result<Self, LoopOutServiceError> {
        let min_amount = cfg.get_int("loopout.min").map_err(|e| {
            LoopOutServiceError::new(format!("error getting loopout.min from config: {}", e))
        })?;
        let max_amount = cfg.get_int("loopout.max").map_err(|e| {
            LoopOutServiceError::new(format!("error getting loopout.max from config: {}", e))
        })?;
        // TODO: double map unideal
        let cltv_delta = cfg
            .get_int("loopout.cltv")
            .map_err(|e| {
                LoopOutServiceError::new(format!("error getting loopout.cltv from config: {}", e))
            })?
            .try_into()
            .map_err(|e| {
                LoopOutServiceError::new(format!("error converting loopout.cltv to u64: {}", e))
            })?;
        let fee_pct = cfg.get_int("loopout.fee").map_err(|e| {
            LoopOutServiceError::new(format!("error getting loopout.fee from config: {}", e))
        })?;

        Ok(Self {
            cfg: LoopOutConfig {
                min_amount,
                max_amount,
                cltv_delta,
                fee_pct,
            },
            db,
            secp256k1: Secp256k1::new(),
            network: wallet.get_network(),
            wallet: Mutex::new(wallet),
            lnd_gateway: Mutex::new(lnd_gateway),
        })
    }

    pub fn get_loop_out(
        &self,
        payment_hash: String,
    ) -> Result<FullLoopOutData, LoopOutServiceError> {
        let conn = &mut self.db.get_conn().map_err(|e| {
            LoopOutServiceError::new(format!("error getting db connection: {:?}", e))
        })?;

        db::get_full_loop_out(conn, payment_hash).map_err(|e| {
            LoopOutServiceError::new(format!("error getting loop_out from db: {:?}", e))
        })
    }

    pub async fn handle_loop_out_request(
        &self,
        pubkey: String,
        amount: i64,
    ) -> Result<FullLoopOutData, LoopOutServiceError> {
        self.validate_amount(amount)?;
        self.validate_pubkey(&pubkey)?;
        log::info!("validated request");
        let buyer_pubkey: XOnlyPublicKey = XOnlyPublicKey::from_str(&pubkey).map_err(|e| {
            LoopOutServiceError::new(format!("error converting pubkey to XOnlyPublicKey: {}", e))
        })?;
        let fee = self.calculate_loop_out_fee(&amount);
        let invoice_amount = amount + fee;

        let conn = &mut self.db.get_conn().map_err(|e| {
            LoopOutServiceError::new(format!("error getting db connection: {:?}", e))
        })?;

        let loop_out = self.add_loop_out(conn)?;

        let invoice = self.add_invoice(conn, &loop_out.id, invoice_amount).await?;

        let script = self
            .add_onchain_htlc(conn, &loop_out.id, &buyer_pubkey, &invoice.payment_hash)
            .await?;

        // TODO: save to dB BEFORE broadcasting tx
        let (utxo, tx) = self
            .add_utxo_to_htlc(conn, script.id, &script.address, amount as u64)
            .await?;

        let full_loop_out_data = db::new_full_loop_out_data(loop_out, invoice, script, utxo);

        match self.broadcast_tx(&tx).await {
            Ok(_) => {
                log::info!("broadcasted tx");
                Ok(full_loop_out_data)
            }
            Err(e) => Err(e),
        }
    }

    // unused for now, but a first attempt at db transactions
    #[allow(dead_code)]
    async fn do_loop_out_request(
        &self,
        conn: &mut db::PooledConnection,
        utxo_amount: i64,
        _fee: i64,
        invoice_amount: i64,
        buyer_pubkey: XOnlyPublicKey,
    ) -> Result<FullLoopOutData, LoopOutServiceError> {
        let loop_out = self.add_loop_out(conn)?;

        let invoice = self.add_invoice(conn, &loop_out.id, invoice_amount).await?;

        let script = self
            .add_onchain_htlc(conn, &loop_out.id, &buyer_pubkey, &invoice.payment_hash)
            .await?;

        // TODO: save to dB BEFORE broadcasting tx
        let (utxo, tx) = self
            .add_utxo_to_htlc(conn, script.id, &script.address, utxo_amount as u64)
            .await?;

        let full_loop_out_data = db::new_full_loop_out_data(loop_out, invoice, script, utxo);

        match self.broadcast_tx(&tx).await {
            Ok(_) => {
                log::info!("broadcasted tx");
                Ok(full_loop_out_data)
            }
            Err(e) => Err(e),
        }
    }

    fn add_loop_out(
        &self,
        conn: &mut db::PooledConnection,
    ) -> Result<models::LoopOut, LoopOutServiceError> {
        let new_loop_out = models::NewLoopOut {
            state: models::LOOP_OUT_STATE_INITIATED.to_string(),
        };

        db::insert_loop_out(conn, new_loop_out).map_err(|e| {
            LoopOutServiceError::new(format!("error inserting loop_out into db: {:?}", e))
        })
    }

    async fn add_invoice(
        &self,
        conn: &mut db::PooledConnection,
        loop_out_id: &i64,
        amount: i64,
    ) -> Result<Invoice, LoopOutServiceError> {
        log::info!("adding invoice...");
        let lndg = self.lnd_gateway.lock().await;
        let invoice = lndg
            .add_invoice(amount)
            .await
            .map_err(|e| LoopOutServiceError::new(format!("error adding invoice: {:?}", e)))?;
        mem::drop(lndg);
        log::info!("added invoice: {:?}", invoice.payment_hash);

        let new_invoice = NewInvoice {
            loop_out_id: *loop_out_id,
            payment_request: &invoice.invoice,
            payment_hash: &invoice.payment_hash,
            payment_preimage: Some(&invoice.preimage),
            amount,
            state: models::INVOICE_STATE_OPEN.to_string(),
        };

        db::insert_invoice(conn, new_invoice).map_err(|e| {
            LoopOutServiceError::new(format!("error inserting invoice into db: {:?}", e))
        })
    }

    async fn add_onchain_htlc(
        &self,
        conn: &mut db::PooledConnection,
        loop_out_id: &i64,
        buyer_pubkey: &XOnlyPublicKey,
        payment_hash: &String,
    ) -> Result<Script, LoopOutServiceError> {
        // Lock wallet here and get all necessary info
        let wallet = self.wallet.lock().await;
        // create new pubkey
        // TODO: use max local_pubkey_index from db
        let (looper_pubkey, looper_pubkey_idx) = (*wallet).new_pubkey().map_err(|e| {
            LoopOutServiceError::new(format!("error generating new pubkey: {:?}", e))
        })?;
        // TODO: sync here to get proper height?
        let curr_height = (*wallet).get_height().map_err(|e| {
            LoopOutServiceError::new(format!("error getting wallet height: {:?}", e))
        })?;
        log::info!("curr_height: {}", curr_height);
        let cltv_delta: u32 = self.cfg.cltv_delta.try_into().map_err(|e| {
            LoopOutServiceError::new(format!("error converting cltv_delta to u32: {}", e))
        })?;
        // TODO: currently unused. bad
        let cltv_expiry_i32 = curr_height + cltv_delta;
        mem::drop(wallet);
        // Unlock wallet

        let mut payhash_bytes = [0u8; 32];
        hex::decode_to_slice(payment_hash, &mut payhash_bytes as &mut [u8]).map_err(|e| {
            LoopOutServiceError::new(format!("error decoding payment_hash: {:?}", e))
        })?;

        let (tr, tweak) = LooperWallet::new_htlc(
            *buyer_pubkey,
            looper_pubkey,
            &payhash_bytes,
            cltv_expiry_i32,
        )
        .map_err(|e| LoopOutServiceError::new(format!("error creating htlc: {:?}", e)))?;

        let address = self.p2tr_address(&tr);

        let cltv_expiry_u32 = cltv_expiry_i32.try_into().map_err(|e| {
            LoopOutServiceError::new(format!("error converting cltv_expiry to u32: {}", e))
        })?;

        // TODO: factor into function
        let new_script = NewScript {
            loop_out_id: *loop_out_id,
            address: &address.to_string(),
            external_tapkey: &tr.output_key().to_string(),
            internal_tapkey: &tr.internal_key().to_string(),
            internal_tapkey_tweak: &hex::encode(tweak.secret_bytes()),
            payment_hash,
            tree: tree_to_vec(&tr),
            cltv_expiry: cltv_expiry_u32,
            remote_pubkey: buyer_pubkey.to_string(),
            local_pubkey: looper_pubkey.to_string(),
            local_pubkey_index: looper_pubkey_idx as i32,
        };

        log::info!("adding script...");
        let res = db::insert_script(conn, new_script).map_err(|e| {
            LoopOutServiceError::new(format!("error inserting script into db: {:?}", e))
        });

        log::info!("added script");

        res
    }

    async fn add_utxo_to_htlc(
        &self,
        conn: &mut db::PooledConnection,
        script_id: i64,
        address: &str,
        amount: u64,
    ) -> Result<(Utxo, bitcoin::Transaction), LoopOutServiceError> {
        log::info!("adding utxo...");
        let tx = self.build_tx_to_address(address, amount).await?;
        let txid = tx.txid();
        let amount = amount.try_into().map_err(|e| {
            LoopOutServiceError::new(format!("error converting amount to u64: {}", e))
        })?;
        let utxo = NewUTXO {
            txid: &txid.to_string(),
            // TODO: fix
            vout: 0,
            amount,
            script_id,
        };
        let utxo = db::insert_utxo(conn, utxo).map_err(|e| {
            LoopOutServiceError::new(format!("error inserting utxo into db: {:?}", e))
        })?;

        log::info!("added utxo {:?}:{:?}", utxo.txid, utxo.vout);

        Ok((utxo, tx))
    }

    async fn build_tx_to_address(
        &self,
        address: &str,
        amount: u64,
    ) -> Result<bitcoin::Transaction, LoopOutServiceError> {
        let wallet = self.wallet.lock().await;
        log::info!("estimating fee rate...");
        let fee_rate = mempool::get_mempool_fee_rate(mempool::MempoolFeePriority::Blocks6)
            .await
            .map_err(|e| LoopOutServiceError::new(format!("error estimating fee rate: {:?}", e)))?;

        log::info!("building tx...");
        let tx = &wallet
            .send_to_address(address, amount, &fee_rate)
            .map_err(|e| {
                LoopOutServiceError::new(format!(
                    "error building tx to address {}: {:?}",
                    address, e
                ))
            })?;
        mem::drop(wallet);
        log::info!("built tx");
        Ok(tx.clone())
    }

    async fn broadcast_tx(&self, tx: &bitcoin::Transaction) -> Result<(), LoopOutServiceError> {
        log::info!("broadcasting tx...");
        let wallet = self.wallet.lock().await;
        let res = (*wallet)
            .broadcast_tx(tx)
            .map_err(|e| LoopOutServiceError::new(format!("error broadcasting tx: {:?}", e)));

        log::info!("broadcasted tx");

        res
    }

    fn p2tr_address(&self, tr: &TaprootSpendInfo) -> Address {
        Address::p2tr(
            &self.secp256k1,
            tr.internal_key(),
            tr.merkle_root(),
            self.network,
        )
    }

    fn calculate_loop_out_fee(&self, amount: &i64) -> i64 {
        amount * self.cfg.fee_pct / 100
    }

    pub fn validate_amount(&self, amount: i64) -> Result<(), LoopOutServiceError> {
        if amount < self.cfg.min_amount {
            return Err(LoopOutServiceError::new("amount too low".to_string()));
        }

        if amount > self.cfg.max_amount {
            return Err(LoopOutServiceError::new("amount too high".to_string()));
        }
        Ok(())
    }

    pub fn validate_pubkey(&self, pubkey_str: &str) -> Result<(), LoopOutServiceError> {
        match XOnlyPublicKey::from_str(pubkey_str) {
            Ok(_) => Ok(()),

            Err(e) => Err(LoopOutServiceError::new(format!(
                "invalid pubkey: {:?}",
                e.to_string()
            ))),
        }
    }
}

pub fn tree_to_vec(tsi: &TaprootSpendInfo) -> Vec<String> {
    let mut vec: Vec<String> = vec![];
    let iter = tsi.as_script_map().iter();

    for ((script, _version), _) in iter {
        vec.push(script.to_hex_string());
    }

    vec
}

#[derive(Debug)]
pub struct LoopOutServiceError {
    pub message: String,
}

impl LoopOutServiceError {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}
