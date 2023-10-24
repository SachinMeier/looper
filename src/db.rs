use crate::settings;
use diesel::prelude::*;

pub fn connect(cfg: &settings::Config) -> PgConnection {
    // TODO: use config
    let host = cfg.get_string("db.host").unwrap();
    let port = cfg.get_int("db.port").unwrap();
    let user = cfg.get_string("db.user").unwrap();
    let pass = cfg.get_string("db.pass").unwrap();
    let name = cfg.get_string("db.name").unwrap();

    let database_url = build_db_connection_string(&host, &port.to_string(), &user, &pass, &name);
    PgConnection::establish(&database_url).expect(&format!("Error connecting to {}", database_url))
}

fn build_db_connection_string(
    host: &str,
    port: &str,
    user: &str,
    pass: &str,
    name: &str,
) -> String {
    format!("postgres://{}:{}@{}:{}/{}", user, pass, host, port, name)
}

pub struct DB {
    pub cfg: settings::Config,
}

impl DB {
    pub fn new(cfg: settings::Config) -> Self {
        Self { cfg }
    }

    pub fn new_conn(&self) -> PgConnection {
        connect(&self.cfg)
    }

    pub fn insert_invoice(&self, invoice: NewInvoice) -> Invoice {
        use crate::schema::invoices::dsl::*;

        let conn = self.new_conn();

        diesel::insert_into(invoices)
            .values(&invoice)
            .get_result(&conn)
            .expect("Error saving new invoice")
    }
}
