use std::thread;
use crate::settings;

use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, Mutex,
};

use fedimint_tonic_lnd::Client;


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
    let client = tonic_lnd::connect(
        cfg.address.clone(),
        cfg.cert_path.clone(),
        cfg.macaroon_path.clone(),
    ).await.unwrap();

    return client;
}

pub struct LNDGateway {
    cfg: LNDConfig,
    rx: Mutex<Receiver<(LNDCmd, oneshot::Sender<LNDResult>)>>,
    tx: Sender<(LNDCmd, oneshot::Sender<LNDResult>)>,
    // TODO: stop signal
}

#[derive(Debug)]
pub enum LNDCmd {
    GetInfoReq(tonic_lnd::lnrpc::GetInfoRequest),
}

#[derive(Debug)]
pub enum LNDResult {
    GetInfoResp(tonic_lnd::lnrpc::GetInfoResponse),
    Error { error: tonic_lnd::Error },
}

impl LNDGateway {
    pub fn new() -> Self {
        let app_cfg = settings::build_config().expect("failed to build config");
        let ln_cfg = settings::get_lnd_config(&app_cfg);

        let (tx, rx) = mpsc::channel::<(LNDCmd, oneshot::Sender<LNDResult>)>(16);
        
        return Self {
            cfg: ln_cfg,
            rx: Mutex::new(rx),
            tx: tx,
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
            }).unwrap();

    }

    async fn handle_cmd(client: &mut Client, cmd: LNDCmd, res_tx: oneshot::Sender<LNDResult>) {
        let resp = match cmd {
            LNDCmd::GetInfoReq(tonic_lnd::lnrpc::GetInfoRequest{}) => {
                let resp = client.lightning().get_info(tonic_lnd::lnrpc::GetInfoRequest{}).await;
                match resp {
                    Ok(resp) => LNDResult::GetInfoResp(resp.into_inner()),
                    Err(e) => LNDResult::Error{ error: e },
                }
            }
        };

        res_tx.send(resp).unwrap();
    }

    pub fn get_info() -> Result<tonic_lnd::lnrpc::GetInfoResponse, tonic_lnd::Error> {
        lnd_cmd!{
            LNDCmd::GetInfoReq(tonic_lnd::lnrpc::GetInfoRequest{}),
            LNDResult::GetInfoResp(resp) => Ok(resp)
        }
    }
}