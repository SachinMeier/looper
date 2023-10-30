use crate::models::{
    FullLoopOutData, Invoice, LoopOut, NewInvoice, NewLoopOut, NewScript, NewUTXO, Script, UTXO,
};
use crate::{lnd, settings};
use diesel::{
    deserialize::FromSql,
    pg::{sql_types::Jsonb, Pg, TransactionBuilder},
    prelude::*,
    serialize::{Output, ToSql},
    sql_types::Array,
};
use diesel_async::pg::AsyncPgConnection;
use diesel_async::AsyncConnection;

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

    pub async fn new_async_conn(&self) -> AsyncPgConnection {
        let database_url = build_db_connection_string(&self.cfg);
        AsyncPgConnection::establish(&database_url)
            .await
            .expect(&format!("Error connecting to {}", database_url))
    }

    pub fn new_tx(&self) -> TransactionBuilder<PgConnection> {
        self.new_conn().build_transaction().read_committed()
    }

    // TODO: make an interface (trait) and implement for DB separately?

    // invoices
    pub fn insert_invoice(
        &self,
        conn: &mut PgConnection,
        invoice: NewInvoice,
    ) -> Result<Invoice, diesel::result::Error> {
        use crate::schema::invoices::dsl::*;

        let res = diesel::insert_into(invoices)
            .values(&invoice)
            .returning(invoices::all_columns())
            .get_result(conn)?;

        Ok(res)
    }

    pub fn update_invoice(
        &self,
        conn: &mut PgConnection,
        invoice: Invoice,
    ) -> Result<Invoice, diesel::result::Error> {
        use crate::schema::invoices::dsl::*;

        let res = diesel::update(invoices.find(invoice.id))
            .set(&invoice)
            .returning(invoices::all_columns())
            .get_result(conn)?;

        Ok(res)
    }

    pub fn get_invoice(
        &self,
        conn: &mut PgConnection,
        invoice_id: i64,
    ) -> Result<Invoice, diesel::result::Error> {
        use crate::schema::invoices::dsl::*;

        let invoice = invoices.filter(id.eq(invoice_id)).first::<Invoice>(conn)?;

        Ok(invoice)
    }

    pub fn list_invoices_in_state(
        &self,
        conn: &mut PgConnection,
        invoice_state: String,
    ) -> Result<Vec<Invoice>, diesel::result::Error> {
        use crate::schema::invoices::dsl::*;

        let results = invoices
            .filter(state.eq(invoice_state))
            .load::<Invoice>(conn)?;

        Ok(results)
    }

    // scripts
    pub fn insert_script(
        &self,
        conn: &mut PgConnection,
        script: NewScript,
    ) -> Result<Script, diesel::result::Error> {
        use crate::schema::scripts::dsl::*;

        let res = diesel::insert_into(scripts)
            .values(&script)
            .returning(scripts::all_columns())
            .get_result(conn)?;

        Ok(res)
    }

    pub fn get_script(
        &self,
        conn: &mut PgConnection,
        script_id: i64,
    ) -> Result<Script, diesel::result::Error> {
        use crate::schema::scripts::dsl::*;

        let script = scripts.filter(id.eq(script_id)).first::<Script>(conn)?;

        Ok(script)
    }

    // UTXOs

    pub fn insert_utxo(
        &self,
        conn: &mut PgConnection,
        utxo: NewUTXO,
    ) -> Result<UTXO, diesel::result::Error> {
        use crate::schema::utxos::dsl::*;

        let res = diesel::insert_into(utxos)
            .values(&utxo)
            .returning(utxos::all_columns())
            .get_result(conn)?;

        Ok(res)
    }

    pub fn get_utxo(
        &self,
        conn: &mut PgConnection,
        utxo_id: i64,
    ) -> Result<UTXO, diesel::result::Error> {
        use crate::schema::utxos::dsl::*;

        let utxo = utxos.filter(id.eq(utxo_id)).first::<UTXO>(conn)?;

        Ok(utxo)
    }

    // LoopOuts

    pub fn insert_loop_out(
        &self,
        conn: &mut PgConnection,
        loop_out: NewLoopOut,
    ) -> Result<LoopOut, diesel::result::Error> {
        use crate::schema::loop_outs::dsl::*;

        let res = diesel::insert_into(loop_outs)
            .values(&loop_out)
            .returning(loop_outs::all_columns())
            .get_result(conn)?;

        Ok(res)
    }

    pub fn get_loop_out(
        &self,
        conn: &mut PgConnection,
        loop_out_id: i64,
    ) -> Result<LoopOut, diesel::result::Error> {
        use crate::schema::loop_outs::dsl::*;

        let loop_out = loop_outs
            .filter(id.eq(loop_out_id))
            .first::<LoopOut>(conn)?;

        Ok(loop_out)
    }

    pub fn get_full_loop_out(
        &self,
        conn: &mut PgConnection,
        payment_hash: String,
    ) -> Result<FullLoopOutData, diesel::result::Error> {
        use crate::schema::invoices::{self, dsl::*};
        use crate::schema::loop_outs::{self, dsl::*};
        use crate::schema::scripts::{self, dsl::*};
        use crate::schema::utxos::{self, dsl::*};

        let (loop_out, invoice, script, utxo) = loop_outs
            .left_join(invoices.on(invoices::loop_out_id.eq(loop_outs::id.nullable())))
            .left_join(scripts.on(scripts::loop_out_id.eq(loop_outs::id.nullable())))
            .left_join(utxos.on(utxos::script_id.nullable().eq(scripts::id.nullable())))
            .filter(invoices::payment_hash.eq(payment_hash))
            .first(conn)?;

        match (invoice, script, utxo) {
            (Some(invoice), Some(script), Some(utxo)) => Ok(Self::new_full_loop_out_data(
                loop_out, invoice, script, utxo,
            )),
            _ => Err(diesel::result::Error::NotFound),
        }
    }

    pub fn new_full_loop_out_data(
        loop_out: LoopOut,
        invoice: Invoice,
        script: Script,
        utxo: UTXO,
    ) -> FullLoopOutData {
        FullLoopOutData {
            loop_out,
            invoice,
            script,
            utxo,
        }
    }
}
