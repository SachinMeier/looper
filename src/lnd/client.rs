use crate::settings;
use crate::utils;
use hex;
use std::collections::HashMap;

use tokio::sync::Mutex;

use fedimint_tonic_lnd::{
    invoicesrpc,
    lnrpc::{self, FeeLimit},
    routerrpc, Client,
};

#[derive(Clone)]
pub struct LNDConfig {
    pub address: String,
    pub cert_path: String,
    pub macaroon_path: String,
    pub invoice_lifetime: i64,
}

const DEFAULT_INVOICE_LIFETIME: u64 = 86400;

pub fn get_lnd_config(cfg: &settings::Config) -> Result<LNDConfig, LNDGatewayError> {
    let invoice_lifetime = match cfg.get("lnd.invoice_lifetime") {
        Ok(v) => v,
        Err(_) => DEFAULT_INVOICE_LIFETIME,
    };

    let address = cfg
        .get("lnd.address")
        .map_err(|e| LNDGatewayError::new(format!("lnd.address not set: {:?}", e.to_string())))?;
    let cert_path = cfg
        .get("lnd.cert_path")
        .map_err(|e| LNDGatewayError::new(format!("lnd.cert_path not set: {:?}", e.to_string())))?;
    let macaroon_path = cfg.get("lnd.macaroon_path").map_err(|e| {
        LNDGatewayError::new(format!("lnd.macaroon_path not set: {:?}", e.to_string()))
    })?;

    Ok(LNDConfig {
        address,
        cert_path,
        macaroon_path,
        invoice_lifetime: invoice_lifetime as i64,
    })
}

pub async fn new_client(cfg: LNDConfig) -> Result<Client, fedimint_tonic_lnd::ConnectError> {
    fedimint_tonic_lnd::connect(
        cfg.address.clone(),
        cfg.cert_path.clone(),
        cfg.macaroon_path.clone(),
    )
    .await
}

pub struct LNDGateway {
    cfg: LNDConfig,
    client: Mutex<Client>,
}

#[derive(Debug)]
pub struct AddInvoiceResp {
    pub preimage: String,
    pub payment_hash: String,
    pub invoice: String,
    pub add_index: u64,
}

impl LNDGateway {
    pub async fn new() -> Result<Self, LNDGatewayError> {
        let app_cfg = settings::build_config().expect("failed to build config");
        let ln_cfg = get_lnd_config(&app_cfg)?;

        let client = new_client(ln_cfg.clone()).await.map_err(|e| {
            LNDGatewayError::new(format!(
                "failed to connect to lnd: {:?} {:?}",
                ln_cfg.address, e
            ))
        })?;

        Ok(Self {
            cfg: ln_cfg,
            client: Mutex::new(client),
        })
    }

    async fn get_client(&self) -> tokio::sync::MutexGuard<'_, Client> {
        self.client.lock().await
    }

    pub async fn get_info(&self) -> Result<lnrpc::GetInfoResponse, fedimint_tonic_lnd::Error> {
        let mut client = self.get_client().await;
        let resp = client.lightning().get_info(lnrpc::GetInfoRequest {}).await;
        match resp {
            Ok(resp) => Ok(resp.into_inner()),
            Err(e) => Err(e),
        }
    }

    pub async fn add_invoice(
        &self,
        value: i64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
        let mut client = self.get_client().await;

        // TODO: do we have to generate this?
        let (preimage, payment_hash) = LNDGateway::new_preimage();
        let payment_addr = LNDGateway::new_payment_addr();
        // resolves lint vs compile error dilemma
        #[allow(deprecated)]
        let req = lnrpc::Invoice {
            memo: "looper swap out".to_string(),
            r_preimage: preimage.to_vec(),
            r_hash: payment_hash.to_vec(),
            expiry: self.cfg.invoice_lifetime,
            value,
            value_msat: 0,
            settled: false,
            creation_date: 0,
            settle_date: 0,
            payment_request: "".to_string(),
            description_hash: vec![],
            fallback_addr: "".to_string(),
            // TODO: set this explicitly
            cltv_expiry: 0, // cltv_expiry,
            private: true,
            add_index: 0,
            settle_index: 0,
            amt_paid: 0,
            amt_paid_sat: 0,
            amt_paid_msat: 0,
            state: 0,
            htlcs: vec![],
            features: HashMap::new(),
            is_keysend: false,
            payment_addr: payment_addr.to_vec(),
            is_amp: false,
            amp_invoice_state: HashMap::new(),
            route_hints: vec![],
        };

        let resp = client.lightning().add_invoice(req).await;

        match resp {
            Ok(resp) => {
                let resp = resp.into_inner();
                Ok(AddInvoiceResp {
                    preimage: hex::encode(preimage),
                    payment_hash: hex::encode(payment_hash),
                    invoice: resp.payment_request,
                    add_index: resp.add_index,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub async fn add_hold_invoice(
        &self,
        value: i64,
        cltv_timout: u64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
        let mut client = self.get_client().await;
        let (preimage, payment_hash) = LNDGateway::new_preimage();

        let req = invoicesrpc::AddHoldInvoiceRequest {
            memo: "looper swap out".to_string(),
            hash: payment_hash.to_vec(),
            value,
            value_msat: 0,
            description_hash: vec![],
            expiry: 86000,
            fallback_addr: "".to_string(),
            cltv_expiry: cltv_timout,
            route_hints: vec![],
            private: true,
        };

        let resp = client.invoices().add_hold_invoice(req).await;

        match resp {
            Ok(resp) => {
                let resp = resp.into_inner();
                Ok(AddInvoiceResp {
                    preimage: hex::encode(preimage),
                    payment_hash: hex::encode(payment_hash),
                    invoice: resp.payment_request,
                    add_index: resp.add_index,
                })
            }
            Err(e) => Err(e),
        }
    }

    pub async fn pay_invoice_async(
        &self,
        invoice: String,
        fee_limit: i64,
    ) -> Result<(), fedimint_tonic_lnd::Error> {
        let mut client = self.get_client().await;
        // resolves lint vs compile error dilemma
        #[allow(deprecated)]
        let req = routerrpc::SendPaymentRequest {
            payment_request: invoice,
            timeout_seconds: 600,
            amt: 0,
            amt_msat: 0,
            dest: vec![],
            payment_hash: vec![],
            final_cltv_delta: 0,
            fee_limit_sat: fee_limit,
            fee_limit_msat: 0,
            outgoing_chan_id: 0,
            outgoing_chan_ids: vec![],
            last_hop_pubkey: vec![],
            // TODO: SET ME to cltv_delta + block height so that we can ensure the invoice can't be held
            // until the onchain timelock expires.
            cltv_limit: 0,
            route_hints: vec![],
            dest_custom_records: HashMap::new(),
            allow_self_payment: false,
            dest_features: vec![],
            max_parts: 64,
            no_inflight_updates: true,
            payment_addr: vec![],
            max_shard_size_msat: 0,
            amp: false,
            time_pref: -1.0,
        };

        let resp = client.router().send_payment_v2(req).await;
        match resp {
            // TODO: return payment stream and start tracking in a separate thread
            Ok(_) => Ok(()),
            Err(e) => Err(e),
        }
    }

    pub async fn pay_invoice_sync(
        &self,
        invoice: String,
        fee_limit: i64,
    ) -> Result<lnrpc::SendResponse, fedimint_tonic_lnd::Error> {
        let mut client = self.get_client().await;
        // resolves lint vs compile error dilemma
        #[allow(deprecated)]
        let req = lnrpc::SendRequest {
            dest: vec![],
            dest_string: "".to_string(),
            amt: 0,
            amt_msat: 0,
            payment_hash: vec![],
            payment_hash_string: "".to_string(),
            payment_request: invoice,
            final_cltv_delta: 0,
            fee_limit: Some(FeeLimit {
                limit: Some(lnrpc::fee_limit::Limit::Fixed(fee_limit)),
            }),
            outgoing_chan_id: 0,
            last_hop_pubkey: vec![],
            cltv_limit: 0,
            dest_custom_records: HashMap::new(),
            allow_self_payment: false,
            dest_features: vec![],
            payment_addr: vec![],
        };

        let resp = client.lightning().send_payment_sync(req).await;

        match resp {
            Ok(resp) => Ok(resp.into_inner()),
            Err(e) => Err(e),
        }
    }

    pub fn new_preimage() -> ([u8; 32], [u8; 32]) {
        let preimage: [u8; 32] = utils::rand_32_bytes();
        let payment_hash = utils::sha256(&preimage);

        (preimage, payment_hash)
    }

    // TODO move this to a general new_32_byte_array function
    pub fn new_payment_addr() -> [u8; 32] {
        utils::rand_32_bytes()
    }
}

// TODO: should we make every method return this error or is fedimint_tonic_lnd::Error sufficient?
#[derive(Debug)]
pub struct LNDGatewayError {
    pub msg: String,
}

impl LNDGatewayError {
    pub fn new(msg: String) -> Self {
        Self { msg }
    }
}
