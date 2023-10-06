use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use std::thread;

use crate::services::{
    errors::LooperError,
    loop_out::{LoopOutRequest, LoopOutResponse},
};

pub struct Response {
    pub status: u16,
    pub error: Option<LooperError>,
    pub data: Option<LoopResponse>,
}

pub fn start() {
    rocket();
}

fn rocket() {
    let rt = thread::Builder::new()
        .name("rocket-api".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            rt.block_on(async move {
                let builder = rocket::build().mount("/", routes![index, loop_out,]);
                let _ = builder.launch().await;
            });
        })
        .unwrap();
}

#[get("/")]
pub fn index() -> &'static str {
    "Hello, world!"
}

#[post("/out", format = "json", data = "<loop_out>")]
pub fn loop_out(loop_out: Json<LoopOutRequest>) -> Json<LoopOutResponse> {
    // let ex_resp = LoopOutResponse {
    //     pubkey: "pubkey".to_string(),
    //     tweak: "tweak".to_string(),
    //     taproot_script_info: TaprootScriptInfo {
    //         external_key: "external_key".to_string(),
    //         internal_key: "internal_key".to_string(),
    //         tree: vec!["script1".to_string(), "script2".to_string()],
    //     },
    //     loop_info: LoopOutInfo {
    //         fee: 100,
    //     },
    //     invoice: "invoice".to_string(),
    // };
    match services::LoopOutService::handle_loop_out_request(loop_out) {
        Ok(resp) => Json(resp),
        Err(resp) => Json(resp),
    }
}
