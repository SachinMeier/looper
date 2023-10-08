use crate::services::{
    errors::LooperError,
    loop_out::{LoopOutRequest, LoopOutResponse, LoopOutService},
};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::tokio;
use std::thread;

// pub struct Response {
//     pub status: u16,
//     pub error: Option<LooperError>,
//     pub data: Option<LoopResponse>,
// }

pub struct LooperServer {
    pub loop_out_svc: LoopOutService,
}

impl LooperServer {
    pub fn new(loop_out_svc: LoopOutService) -> Self {
        Self { loop_out_svc }
    }
    pub fn start(self) {
        self.serve();
    }

    fn serve(self) {
        let rt = thread::Builder::new()
            .name("rocket-api".to_string())
            .spawn(async move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    let builder = rocket::build()
                        .manage(self.loop_out_svc)
                        .mount("/loop", routes![index, loop_out]);
                    let _ = builder.launch().await;
                });
            })
            .unwrap();
    }
}

#[get("/")]
pub fn index() -> &'static str {
    "Hello, world!"
}

#[post("/out", format = "json", data = "<loop_out>")]
pub fn loop_out(
    loop_out_svc: &rocket::State<LoopOutService>,
    loop_out: Json<LoopOutRequest>,
) -> Json<LoopOutResponse> {
    let resp = loop_out_svc.handle_loop_out_request(loop_out.into_inner());

    Json(resp)
}
