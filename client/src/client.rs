use looper::services::loop_out::{LoopOutRequest, LoopOutResponse};
use reqwest::blocking;

pub struct Config {
    pub url: String,
}

pub struct LooperClient {
    pub cfg: Config,
    pub client: blocking::Client,
}

impl LooperClient {
    pub fn new(cfg: Config) -> Self {
        Self {
            cfg: cfg,
            client: blocking::Client::new(),
        }
    }

    pub fn new_loop_out(&self, pubkey: String, amount: i64) -> LoopOutResponse {
        let url = format!("{}/loop/out", self.cfg.url);
        let req = LoopOutRequest {
            pubkey: pubkey,
            amount: amount,
        };
        let res = self.client.post(url).json(&req).send().unwrap();

        res.json::<LoopOutResponse>().unwrap()
    }
}
