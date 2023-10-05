use std::env;
use std::sync::Once;
use crate::client::LNDConfig;

use config::{Config, ConfigError, Environment, File};

static INIT: Once = Once::new();

pub fn build_config() -> Result<Config, ConfigError> {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "local".into());
    log::info!("loading config for {}", profile);

    Config::builder()
        .add_source(File::with_name("config/default"))
        .add_source(File::with_name(&format!("config/{}", profile)).required(false))
        .add_source(Environment::with_prefix("RLS").separator("_"))
        .build()
}

pub fn get_lnd_config(cfg: &Config) -> LNDConfig {
    LNDConfig{
        address: cfg.get("lnd.address").unwrap(),
        cert_path: cfg.get("lnd.cert_path").unwrap(),
        macaroon_path: cfg.get("lnd.macaroon_path").unwrap(),
    }
}

pub fn init_logging() {
    INIT.call_once(|| {
        let _ = log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    });
}
