use crate::models::{Invoice, NewInvoice};
use crate::settings;
use diesel::prelude::*;

pub struct DBConfig {
    pub host: String,
    pub port: i64,
    pub user: String,
    pub pass: String,
    pub name: String,
}

fn get_db_config(cfg: &settings::Config) -> DBConfig {
    let host = cfg.get_string("db.host").unwrap();
    let port = cfg.get_int("db.port").unwrap();
    let user = cfg.get_string("db.user").unwrap();
    let pass = cfg.get_string("db.pass").unwrap();
    let name = cfg.get_string("db.name").unwrap();

    DBConfig {
        host,
        port,
        user,
        pass,
        name,
    }
}

pub fn connect(cfg: &DBConfig) -> PgConnection {
    let database_url = build_db_connection_string(cfg);
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

fn build_db_connection_string(cfg: &DBConfig) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        &cfg.user, &cfg.pass, &cfg.host, &cfg.port, &cfg.name
    )
}

pub struct DB {
    pub cfg: DBConfig,
}

impl DB {
    pub fn new(cfg: &settings::Config) -> Self {
        let db_cfg = get_db_config(cfg);
        Self { cfg: db_cfg }
    }

    pub fn new_conn(&self) -> PgConnection {
        connect(&self.cfg)
    }

    // todo: return Invoice?
    pub fn insert_invoice(&self, invoice: NewInvoice) -> Result<(), diesel::result::Error> {
        use crate::schema::invoices::dsl::*;

        let mut conn = self.new_conn();

        diesel::insert_into(invoices)
            .values(&invoice)
            .execute(&mut conn)?;

        Ok(())
    }
}
