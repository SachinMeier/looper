use crate::{
    api::{
        self,
        errors::{self, LooperErrorResponse},
        LoopOutRequest, LoopOutResponse,
    },
    services::loop_out::LoopOutService,
};
use rocket::serde::json::Json;
use std::thread;

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
        thread::Builder::new()
            .name("rocket-api".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                rt.block_on(async move {
                    // TODO: build custom with config & timeout
                    let builder = rocket::build()
                        .manage(self.loop_out_svc)
                        .mount("/loop", routes![index, new_loop_out, get_loop_out]);
                    let _ = builder.launch().await;
                });
            })
            .unwrap();
    }

    fn validate_loop_out_request(
        loop_out_svc: &LoopOutService,
        req: &LoopOutRequest,
    ) -> Result<(), LooperErrorResponse> {
        loop_out_svc.validate_amount(req.amount).map_err(|e| {
            let msg = format!("invalid amount: {:?}", e);
            log::info!("{}", &msg);
            errors::invalid_parameter("amount".to_string())
        })?;

        loop_out_svc.validate_pubkey(&req.pubkey).map_err(|e| {
            let msg = format!("invalid pubkey: {:?}", e);
            log::info!("{}", &msg);
            errors::invalid_parameter("pubkey".to_string())
        })
    }

    fn validate_payment_hash(pay_hash: &String) -> Result<(), LooperErrorResponse> {
        // TODO: avoid instantiating this every time. make const or only instantiate if err?
        let err_invalid = errors::invalid_parameter("payment_hash".to_string());

        let bytes = hex::decode(pay_hash).map_err(|e| {
            log::info!("invalid payment hash: {:?}", e);
            err_invalid.clone()
        })?;
        match bytes.len() {
            32 => Ok(()),

            _ => {
                log::info!("invalid payment hash: {:?}", bytes);
                Err(err_invalid)
            }
        }
    }
}

#[get("/")]
pub fn index() -> &'static str {
    "Hello, world!"
}

#[post("/out", format = "json", data = "<loop_out>")]
pub async fn new_loop_out(
    loop_out_svc: &rocket::State<LoopOutService>,
    loop_out: Json<LoopOutRequest>,
) -> Result<Json<LoopOutResponse>, LooperErrorResponse> {
    let req = loop_out.into_inner();
    LooperServer::validate_loop_out_request(loop_out_svc.inner(), &req)?;

    let resp = loop_out_svc
        .handle_loop_out_request(req.pubkey, req.amount)
        .await
        .map_err(errors::handle_loop_out_error)?;

    Ok(Json(api::map_loop_out_data_to_response(resp)))
}

#[get("/out/<payment_hash>")]
pub fn get_loop_out(
    loop_out_svc: &rocket::State<LoopOutService>,
    payment_hash: String,
) -> Result<Json<LoopOutResponse>, LooperErrorResponse> {
    LooperServer::validate_payment_hash(&payment_hash)?;
    let resp = loop_out_svc
        .get_loop_out(payment_hash)
        .map_err(errors::handle_loop_out_error)?;
    Ok(Json(api::map_loop_out_data_to_response(resp)))
}
