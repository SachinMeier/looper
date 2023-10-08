use crate::settings;
use crate::utils::utils;
use hex;
use rand::Rng;
use std::{collections::HashMap, thread};

use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, Mutex,
};

use fedimint_tonic_lnd::{
    invoicesrpc,
    lnrpc::{self, FeeLimit},
    routerrpc::{self, TrackPaymentRequest},
    Client,
};

#[derive(Clone)]
pub struct LNDConfig {
    pub address: String,
    pub cert_path: String,
    pub macaroon_path: String,
}

pub fn get_lnd_config(cfg: &settings::Config) -> LNDConfig {
    LNDConfig {
        address: cfg.get("lnd.address").unwrap(),
        cert_path: cfg.get("lnd.cert_path").unwrap(),
        macaroon_path: cfg.get("lnd.macaroon_path").unwrap(),
    }
}

lazy_static::lazy_static! {
    static ref GATEWAY: LNDGateway = LNDGateway::new();
}

macro_rules! lnd_cmd {
    ($cmd:expr, $($matcher:pat => $result:expr),*) => {
        let (tx, rx) = oneshot::channel();
        GATEWAY.tx.blocking_send(($cmd, tx)).unwrap();
        match rx.blocking_recv().unwrap() {
            $($matcher => $result,)*
            LNDResult::Error { error } => Err(error),
            _ => panic!(),
        }
    };
}

pub async fn new_client(cfg: LNDConfig) -> Client {
    fedimint_tonic_lnd::connect(
        cfg.address.clone(),
        cfg.cert_path.clone(),
        cfg.macaroon_path.clone(),
    )
    .await
    .unwrap()
}

pub struct LNDGateway {
    cfg: LNDConfig,
    rx: Mutex<Receiver<(LNDCmd, oneshot::Sender<LNDResult>)>>,
    tx: Sender<(LNDCmd, oneshot::Sender<LNDResult>)>,
    // TODO: stop signal
}

#[derive(Debug)]
pub struct AddInvoiceResp {
    pub preimage: String,
    pub payment_hash: String,
    pub invoice: String,
    pub add_index: u64,
}

#[derive(Debug)]
pub enum LNDCmd {
    GetInfoReq(lnrpc::GetInfoRequest),
    AddInvoiceReq(lnrpc::Invoice),
    AddHoldInvoiceReq(invoicesrpc::AddHoldInvoiceRequest),
    SendPaymentReq(routerrpc::SendPaymentRequest),
    SendPaymentSyncReq(lnrpc::SendRequest),
    // TODO: TrackPaymentReq(Vec<u8>),
}

#[derive(Debug)]
pub enum LNDResult {
    GetInfoResp(lnrpc::GetInfoResponse),
    AddInvoiceResp(lnrpc::AddInvoiceResponse),
    AddHoldInvoiceResp(invoicesrpc::AddHoldInvoiceResp),
    SendPaymentResp,
    SendPaymentSyncResp(lnrpc::SendResponse),
    // TrackPaymentResp(),
    Error { error: fedimint_tonic_lnd::Error },
}

impl LNDGateway {
    pub fn new() -> Self {
        let app_cfg = settings::build_config().expect("failed to build config");
        let ln_cfg = get_lnd_config(&app_cfg);

        let (tx, rx) = mpsc::channel::<(LNDCmd, oneshot::Sender<LNDResult>)>(16);

        Self {
            cfg: ln_cfg,
            rx: Mutex::new(rx),
            tx,
        }
    }

    pub fn start() {
        thread::Builder::new()
            .name("lndgateway".to_string())
            .spawn(move || {
                log::info!("starting lnd gateway");
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    let mut client = new_client(GATEWAY.cfg.clone()).await;

                    loop {
                        let (cmd, res_tx) = GATEWAY.rx.lock().await.recv().await.unwrap();
                        LNDGateway::handle_cmd(&mut client, cmd, res_tx).await;
                    }
                })
            })
            .unwrap();
    }

    async fn handle_cmd(client: &mut Client, cmd: LNDCmd, res_tx: oneshot::Sender<LNDResult>) {
        let resp = match cmd {
            LNDCmd::GetInfoReq(lnrpc::GetInfoRequest {}) => {
                let resp = client.lightning().get_info(lnrpc::GetInfoRequest {}).await;
                match resp {
                    Ok(resp) => LNDResult::GetInfoResp(resp.into_inner()),
                    Err(e) => LNDResult::Error { error: e },
                }
            }

            LNDCmd::AddInvoiceReq(invoice) => {
                let resp = client.lightning().add_invoice(invoice).await;

                match resp {
                    Ok(resp) => LNDResult::AddInvoiceResp(resp.into_inner()),
                    Err(e) => LNDResult::Error { error: e },
                }
            }

            LNDCmd::AddHoldInvoiceReq(invoice_req) => {
                let resp = client.invoices().add_hold_invoice(invoice_req).await;

                match resp {
                    Ok(resp) => LNDResult::AddHoldInvoiceResp(resp.into_inner()),
                    Err(e) => LNDResult::Error { error: e },
                }
            }

            LNDCmd::SendPaymentReq(payment_req) => {
                let resp = client.router().send_payment_v2(payment_req).await;

                match resp {
                    // TODO: return payment stream and start tracking in a separate thread
                    Ok(_) => LNDResult::SendPaymentResp,
                    Err(e) => LNDResult::Error { error: e },
                }
            }

            LNDCmd::SendPaymentSyncReq(payment_req) => {
                let resp = client.lightning().send_payment_sync(payment_req).await;

                match resp {
                    Ok(resp) => LNDResult::SendPaymentSyncResp(resp.into_inner()),
                    Err(e) => LNDResult::Error { error: e },
                }
            } // LNDCmd::TrackPayment(payment_hash) => {
              //     let req = TrackPaymentRequest{
              //         payment_hash,
              //         no_inflight_updates: false
              //     }
              //     let resp = client.router().track_payment_v2(req).await;

              //     match resp {
              //         Ok(resp) => LNDResult::TrackPaymentResp(resp.into_inner()),
              //         Err(e) => LNDResult::Error { error: e },
              //     }
              // }
        };

        res_tx.send(resp).unwrap();
    }

    pub fn get_info() -> Result<lnrpc::GetInfoResponse, fedimint_tonic_lnd::Error> {
        lnd_cmd! {
            LNDCmd::GetInfoReq(lnrpc::GetInfoRequest{}),
            LNDResult::GetInfoResp(resp) => Ok(resp)
        }
    }

    pub fn add_invoice(
        value: i64,
        _cltv_expiry: u64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
        // TODO: do we have to generate this?
        let (preimage, payment_hash) = LNDGateway::new_preimage();
        let payment_addr = LNDGateway::new_payment_addr();
        let req = lnrpc::Invoice {
            memo: "looper swap out".to_string(),
            r_preimage: preimage.to_vec(),
            r_hash: payment_hash.to_vec(),
            value,
            value_msat: 0,
            settled: false,
            creation_date: 0,
            settle_date: 0,
            payment_request: "".to_string(),
            description_hash: vec![],
            expiry: 86000,
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

        lnd_cmd! {
            LNDCmd::AddInvoiceReq(req),
            LNDResult::AddInvoiceResp(resp) => Ok(AddInvoiceResp{
                    preimage: hex::encode(preimage),
                    payment_hash: hex::encode(payment_hash),
                    invoice: resp.payment_request,
                    add_index: resp.add_index,
                })
        }
    }

    pub fn add_hold_invoice(
        value: i64,
        cltv_timout: u64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
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

        lnd_cmd! {
            LNDCmd::AddHoldInvoiceReq(req),
            LNDResult::AddHoldInvoiceResp(resp) => Ok(AddInvoiceResp{
                    preimage: hex::encode(preimage),
                    payment_hash: hex::encode(payment_hash),
                    invoice: resp.payment_request,
                    add_index: resp.add_index,
                })
        }
    }

    pub fn pay_invoice_async(
        invoice: String,
        fee_limit: i64,
    ) -> Result<(), fedimint_tonic_lnd::Error> {
        // TODO: conditional logic for amt vs zero
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

        lnd_cmd! {
            LNDCmd::SendPaymentReq(req),
            LNDResult::SendPaymentResp => Ok(())
        }
    }

    pub fn pay_invoice_sync(
        invoice: String,
        fee_limit: i64,
    ) -> Result<lnrpc::SendResponse, fedimint_tonic_lnd::Error> {
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

        lnd_cmd! {
            LNDCmd::SendPaymentSyncReq(req),
            LNDResult::SendPaymentSyncResp(resp) => Ok(resp)
        }
    }

    // pub fn track_payment(
    //     payment_hash: String,
    // ) -> Result<lnrpc::TrackPaymentResponse, fedimint_tonic_lnd::Error> {
    //     let req = lnrpc::TrackPaymentRequest {
    //         payment_hash: payment_hash.into_bytes(),
    //     };

    //     lnd_cmd! {
    //         LNDCmd::TrackPaymentReq(req),
    //         LNDResult::TrackPaymentResp(resp) => Ok(resp)
    //     }
    // }

    pub fn new_preimage() -> ([u8; 32], [u8; 32]) {
        let mut rng = rand::thread_rng();

        let mut preimage: [u8; 32] = [0; 32];
        for i in 0..32 {
            preimage[i] = rng.gen::<u8>();
        }
        let payment_hash = utils::sha256(&preimage);

        (preimage, payment_hash)
    }

    // TODO move this to a general new_32_byte_array function
    pub fn new_payment_addr() -> [u8; 32] {
        let mut rng = rand::thread_rng();

        let mut payment_addr: [u8; 32] = [0; 32];
        for i in 0..32 {
            payment_addr[i] = rng.gen::<u8>();
        }

        payment_addr
    }
}
