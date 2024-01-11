use std::env;
use std::sync::Once;

use config::{Environment, File};

pub use config::{Config, ConfigError};

static INIT: Once = Once::new();

pub fn build_config() -> Result<Config, ConfigError> {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "local".into());
    log::info!("loading config for {}", profile);

    Config::builder()
        .add_source(File::with_name("config/example"))
        .add_source(File::with_name(&format!("config/{}", profile)).required(false))
        .add_source(Environment::with_prefix("RLS").separator("_"))
        .build()
}

pub fn build_test_config() -> Result<Config, ConfigError> {
    let profile = env::var("PROFILE").unwrap_or_else(|_| "local".into());
    log::info!("loading config for {}", profile);

    Config::builder()
        .add_source(File::with_name("config/test"))
        .add_source(File::with_name(&format!("config/{}", profile)).required(false))
        .add_source(Environment::with_prefix("RLS").separator("_"))
        .build()
}

pub fn init_logging() {
    INIT.call_once(|| {
        log4rs::init_file("config/log4rs.yaml", Default::default()).unwrap();
    });
}
