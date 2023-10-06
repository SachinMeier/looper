use crate::settings;
use crate::utils::utils;
use hex;
use rand::Rng;
use std::{collections::HashMap, thread};

use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, Mutex,
};

use fedimint_tonic_lnd::{invoicesrpc, lnrpc, routerrpc, Client};

#[derive(Clone)]
pub struct LNDConfig {
    pub address: String,
    pub cert_path: String,
    pub macaroon_path: String,
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
}

#[derive(Debug)]
pub enum LNDResult {
    GetInfoResp(lnrpc::GetInfoResponse),
    AddInvoiceResp(lnrpc::AddInvoiceResponse),
    AddHoldInvoiceResp(invoicesrpc::AddHoldInvoiceResp),
    Error { error: fedimint_tonic_lnd::Error },
}

impl LNDGateway {
    pub fn new() -> Self {
        let app_cfg = settings::build_config().expect("failed to build config");
        let ln_cfg = settings::get_lnd_config(&app_cfg);

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
        cltv_timout: u64,
    ) -> Result<AddInvoiceResp, fedimint_tonic_lnd::Error> {
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
            cltv_expiry: cltv_timout,
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
                    invoice: resp.payment_request,
                    add_index: resp.add_index,
                })
        }
    }

    pub fn new_preimage() -> ([u8; 32], [u8; 32]) {
        let mut rng = rand::thread_rng();

        let mut preimage: [u8; 32] = [0; 32];
        for i in 0..32 {
            preimage[i] = rng.gen::<u8>();
        }
        let payment_hash = utils::sha256(&preimage);

        (preimage, payment_hash)
    }

    pub fn new_payment_addr() -> [u8; 32] {
        let mut rng = rand::thread_rng();

        let mut payment_addr: [u8; 32] = [0; 32];
        for i in 0..32 {
            payment_addr[i] = rng.gen::<u8>();
        }

        payment_addr
    }
}
