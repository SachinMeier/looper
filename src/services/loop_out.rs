use bdk::bitcoin::{PublicKey};


#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutRequest {
    pub pubkey: String,
    pub amount: i64,
}

impl LoopOutRequest {
    fn validate() -> Result<(), LooperError> {
        validate_amount(req.amount)?;
        validate_pubkey(req.pubkey)?;
        return Ok(())
    }
}

fn validate_amount(amount: i64) -> Result<(), LooperError> {
    if amount < 0 {
        return Err(LooperError::new(StatusCode::BAD_REQUEST, "amount must be positive".to_string()));
    }
    Ok(())
}

fn validate_pubkey(pubkey_str: String) -> Result<(), LooperError> {
    match PublicKey::from_str(pubkey_str) {
        Ok(_) => Ok(()),

        Err(_) => Err(LooperError::new(StatusCode::BAD_REQUEST, "invalid pubkey".to_string()))

    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct TaprootScriptInfo {
    pub external_key: String,
    pub internal_key: String,
    pub internal_key_tweak: String,
    pub tree: Vec<String>,
}

fn new_taproot_script_info(tsi: TaprootSpendInfo, tweak: SecretKey) -> TaprootScriptInfo {
    TaprootScriptInfo{
        external_key: tsi.output_key().to_string(),
        internal_key: tsi.internal_key().to_string(),
        internal_key_tweak: tweak,
        tree: tree_to_vec(tsi)
    }
}


fn tree_to_vec(tsi: TaprootSpendInfo) -> Vec<String> {
    let vec: Vec<String> = vec![]
    let iter = tsi.as_script_map().into_iter()

    for script in iter {
        vec.append(script.to_hex_string())
    }

    return vec;
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutInfo {
    pub fee: i64,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct LoopOutResponse {
    pub invoice: String,
    pub pubkey: String,
    pub tweak: String,
    pub taproot_script_info: TaprootScriptInfo,
    pub loop_info: LoopOutInfo,
}

pub struct LoopOutConfig {
    pub min_amount: i64,
    pub max_amount: i64,
    pub cltv_timeout: i64,
    pub fee_pct: i64,
}

lazy_static::lazy_static! {
    static ref LOS: LoopOutService = LoopOutService::new();
}


pub struct LoopOutService {
    cfg: LoopOutConfig,
}

impl LoopOutService {
    fn new() -> Self {
        let cfg = settings::build_config().unwrap();

        LoopOutService{
            config: LoopOutConfig{
                min_amount: cfg.get_int("loopout.min").unwrap()
                max_amount: cfg.get_int("loopout.max").unwrap()
                cltv_timeout: cfg.get_int("loopout.cltv").unwrap()
                fee_pct: cfg.get_int("loopout.fee").unwrap()
            }
        }
    }


    pub fn handle_loop_out_request(req: LoopOutRequest) -> Result<LoopOutResponse, LooperError> {
        req.validate()?;
        
        let buyer_pubkey: PublicKey = PublicKey::from_str(req.pubkey).unwrap();
        let fee = LOS.calculate_fee(req.amount);
        let invoice_amount = req.amount + fee;
        // create new pubkey
        let looper_pubkey: PublicKey = PublicKey::from_str("todo implement me").unwrap();
        
        let invoice = LNDGateway::add_invoice(invoice_amount);
        
        let (tr, tweak) = Wallet::new_htlc(buyer_pubkey, looper_pubkey, )

        let taproot_script_info = new_taproot_script_info(tr, tweak)

        // convert tr, tweak to scriptInfo & string respectively

        let resp = LoopOutResponse{
            invoice: invoice,
            pubkey: looper_pubkey,
            tweak: tweak,
            taproot_script_info: taproot_script_info,
            loop_info: loop_info,
        }
        
        return Ok(resp)
    }
    
    fn calculate_fee(&self, amount: i64) -> i64 {
        return amount * self.cfg.fee_pct / 100
    }
}