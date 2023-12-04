use crate::models::{
    FullLoopOutData, Invoice, LoopOut, NewInvoice, NewLoopOut, NewScript, NewUTXO, Script, Utxo,
};
use crate::settings;
use diesel::{
    pg::Pg,
    prelude::*,
    r2d2::{self, ConnectionManager, Pool},
};

// use diesel_async::pg::AsyncPgConnection;
// use diesel_async::AsyncConnection;
use diesel_migrations::{embed_migrations, EmbeddedMigrations, MigrationHarness};

pub struct DBConfig {
    pub host: String,
    pub port: i64,
    pub user: String,
    pub pass: String,
    pub name: String,
}

// Simple type alias for a [Pool] of [PgConnection]s.
pub type ConnectionPool = Pool<ConnectionManager<PgConnection>>;
// Simple type alias for a [PooledConnection] of [PgConnection]s.
pub type PooledConnection =
    r2d2::PooledConnection<diesel::r2d2::ConnectionManager<diesel::PgConnection>>;

// Embeds the database migrations into the binary. This is used to run the migrations on application startup.
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!("migrations");

pub fn create_connection_pool(cfg: &settings::Config) -> ConnectionPool {
    let db_cfg = get_db_config(cfg);
    let database_url = build_db_connection_string(&db_cfg);
    let manager = ConnectionManager::<PgConnection>::new(database_url.clone());
    Pool::builder()
        .max_size(10)
        .build(manager)
        .unwrap_or_else(|_| panic!("failed to create pool for {}", database_url))
}

// Runs the embedded database migrations.
pub fn run_migrations(
    connection: &mut impl MigrationHarness<Pg>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    log::info!("running migrations");
    connection.run_pending_migrations(MIGRATIONS)?;
    Ok(())
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

#[allow(dead_code)]
pub fn connect(cfg: &DBConfig) -> PgConnection {
    let database_url = build_db_connection_string(cfg);
    PgConnection::establish(&database_url)
        .unwrap_or_else(|e| panic!("error connecting to {}: {:?}", database_url, e.to_string()))
}

fn build_db_connection_string(cfg: &DBConfig) -> String {
    format!(
        "postgres://{}:{}@{}:{}/{}",
        &cfg.user, &cfg.pass, &cfg.host, &cfg.port, &cfg.name
    )
}

pub struct DB {
    pub cfg: DBConfig,
    pool: ConnectionPool,
}

impl DB {
    pub fn new(cfg: &settings::Config) -> Self {
        let db_cfg = get_db_config(cfg);
        let pool = create_connection_pool(cfg);
        Self { cfg: db_cfg, pool }
    }

    pub fn get_conn(&self) -> Result<PooledConnection, diesel::r2d2::PoolError> {
        self.pool.get()
    }

    // pub async fn new_async_conn(&self) -> AsyncPgConnection {
    //     let database_url = build_db_connection_string(&self.cfg);
    //     AsyncPgConnection::establish(&database_url)
    //         .await
    //         .expect(&format!("Error connecting to {}", database_url))
    // }

    // pub fn new_tx(&self) -> TransactionBuilder<PgConnection> {
    //     self.new_conn().build_transaction().read_committed()
    // }

    // TODO: make an interface (trait) and implement for DB separately?
}

// invoices
pub fn insert_invoice(
    conn: &mut PooledConnection,
    invoice: NewInvoice,
) -> Result<Invoice, diesel::result::Error> {
    use crate::schema::invoices::dsl::*;

    let res = diesel::insert_into(invoices)
        .values(&invoice)
        .returning(invoices::all_columns())
        .get_result(conn)?;

    Ok(res)
}

#[allow(dead_code)]
pub fn update_invoice(
    conn: &mut PooledConnection,
    invoice: Invoice,
) -> Result<Invoice, diesel::result::Error> {
    use crate::schema::invoices::dsl::*;

    let res = diesel::update(invoices.find(invoice.id))
        .set(&invoice)
        .returning(invoices::all_columns())
        .get_result(conn)?;

    Ok(res)
}

#[allow(dead_code)]
pub fn get_invoice(
    conn: &mut PooledConnection,
    invoice_id: i64,
) -> Result<Invoice, diesel::result::Error> {
    use crate::schema::invoices::dsl::*;

    let invoice = invoices.filter(id.eq(invoice_id)).first::<Invoice>(conn)?;

    Ok(invoice)
}

#[allow(dead_code)]
pub fn list_invoices_in_state(
    conn: &mut PooledConnection,
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
    conn: &mut PooledConnection,
    script: NewScript,
) -> Result<Script, diesel::result::Error> {
    use crate::schema::scripts::dsl::*;

    let res = diesel::insert_into(scripts)
        .values(&script)
        .returning(scripts::all_columns())
        .get_result(conn)?;

    Ok(res)
}

#[allow(dead_code)]
pub fn get_script(
    conn: &mut PooledConnection,
    script_id: i64,
) -> Result<Script, diesel::result::Error> {
    use crate::schema::scripts::dsl::*;

    let script = scripts.filter(id.eq(script_id)).first::<Script>(conn)?;

    Ok(script)
}

// UTXOs

pub fn insert_utxo(
    conn: &mut PooledConnection,
    utxo: NewUTXO,
) -> Result<Utxo, diesel::result::Error> {
    use crate::schema::utxos::dsl::*;

    let res = diesel::insert_into(utxos)
        .values(&utxo)
        .returning(utxos::all_columns())
        .get_result(conn)?;

    Ok(res)
}

#[allow(dead_code)]
pub fn get_utxo(conn: &mut PooledConnection, utxo_id: i64) -> Result<Utxo, diesel::result::Error> {
    use crate::schema::utxos::dsl::*;

    let utxo = utxos.filter(id.eq(utxo_id)).first::<Utxo>(conn)?;

    Ok(utxo)
}

// LoopOuts

pub fn insert_loop_out(
    conn: &mut PooledConnection,
    loop_out: NewLoopOut,
) -> Result<LoopOut, diesel::result::Error> {
    use crate::schema::loop_outs::dsl::*;

    let res = diesel::insert_into(loop_outs)
        .values(&loop_out)
        .returning(loop_outs::all_columns())
        .get_result(conn)?;

    Ok(res)
}

#[allow(dead_code)]
pub fn get_loop_out(
    conn: &mut PooledConnection,
    loop_out_id: i64,
) -> Result<LoopOut, diesel::result::Error> {
    use crate::schema::loop_outs::dsl::*;

    let loop_out = loop_outs
        .filter(id.eq(loop_out_id))
        .first::<LoopOut>(conn)?;

    Ok(loop_out)
}

pub fn get_full_loop_out(
    conn: &mut PooledConnection,
    pay_hash: String,
) -> Result<FullLoopOutData, diesel::result::Error> {
    use crate::schema::invoices::{self, dsl::*};
    use crate::schema::loop_outs::{self, dsl::*};
    use crate::schema::scripts::{self, dsl::*};
    use crate::schema::utxos::{self, dsl::*};

    let (loop_out, invoice, script, utxo) = loop_outs
        .left_join(invoices.on(invoices::loop_out_id.eq(loop_outs::id.nullable())))
        .left_join(scripts.on(scripts::loop_out_id.eq(loop_outs::id.nullable())))
        .left_join(utxos.on(utxos::script_id.nullable().eq(scripts::id.nullable())))
        .filter(invoices::payment_hash.eq(pay_hash))
        .first(conn)?;

    match (invoice, script, utxo) {
        (Some(invoice), Some(script), Some(utxo)) => {
            Ok(new_full_loop_out_data(loop_out, invoice, script, utxo))
        }
        _ => Err(diesel::result::Error::NotFound),
    }
}

pub fn get_loop_outs_by_state(
    conn: &mut PooledConnection,
    state: String,
) -> Result<Vec<LoopOut>, diesel::result::Error> {
    use crate::schema::loop_outs::dsl::*;

    let results = loop_outs.filter(state.eq(state)).load::<LoopOut>(conn)?;

    Ok(results)
}

// unused for now, but a first attempt at db transactions
// Insert Full Loop Out Data
#[allow(dead_code)]
pub fn insert_full_loop_out_data(
    conn: &mut PooledConnection,
    loop_out: NewLoopOut,
    invoice: &mut NewInvoice,
    script: &mut NewScript,
    utxo: &mut NewUTXO,
) -> Result<FullLoopOutData, diesel::result::Error> {
    let loop_out = insert_loop_out(conn, loop_out)?;
    invoice.loop_out_id = loop_out.id;
    let invoice = insert_invoice(conn, invoice.clone())?;
    script.loop_out_id = loop_out.id;
    let script = insert_script(conn, script.clone())?;
    utxo.script_id = script.id;
    let utxo = insert_utxo(conn, utxo.clone())?;

    Ok(new_full_loop_out_data(loop_out, invoice, script, utxo))
}

pub fn new_full_loop_out_data(
    loop_out: LoopOut,
    invoice: Invoice,
    script: Script,
    utxo: Utxo,
) -> FullLoopOutData {
    FullLoopOutData {
        loop_out,
        invoice,
        script,
        utxo,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use crate::{
        db::DB,
        models::{
            self, Invoice, LoopOut, NewInvoice, NewLoopOut, NewScript, NewUTXO, Script, Utxo,
        },
        settings,
    };
    use once_cell::sync::Lazy;
    use std::sync::Once;

    static INIT: Once = Once::new();
    static DB: Lazy<DB> = Lazy::new(|| {
        let cfg = settings::build_test_config().expect("failed to load config");
        DB::new(&cfg)
    });

    fn setup_test_db() {
        INIT.call_once(|| {
            let conn = &mut DB
                .get_conn()
                .expect("failed to create test db connection pool");
            let migration_res = super::run_migrations(conn);
            assert!(migration_res.is_ok());
            truncate_tables(conn);
        });
    }

    fn truncate_tables(conn: &mut super::PooledConnection) {
        use diesel::RunQueryDsl;

        diesel::sql_query("TRUNCATE TABLE loop_outs, invoices, scripts, utxos CASCADE;")
            .execute(conn)
            .expect("failed to truncate tables");
    }

    #[test]
    fn test_insert_and_select_loop_out() {
        setup_test_db();
        let conn = &mut DB.get_conn().expect("failed to get new connection");

        let loop_out = NewLoopOut {
            state: models::LOOP_OUT_STATE_INITIATED.to_string(),
        };

        let inserted_loop_out =
            super::insert_loop_out(conn, loop_out).expect("failed to insert loop out");

        assert_eq!(models::LOOP_OUT_STATE_INITIATED, inserted_loop_out.state);

        // get loop_out from db
        let fetched_loop_out =
            super::get_loop_out(conn, inserted_loop_out.id).expect("failed to get loop out");

        assert_loop_outs_equal(inserted_loop_out, fetched_loop_out);
    }

    fn assert_loop_outs_equal(l1: LoopOut, l2: LoopOut) {
        assert_eq!(l1.id, l2.id);
        assert_eq!(l1.state, l2.state);
    }

    #[test]
    fn test_insert_and_select_full_loop_out() {
        setup_test_db();
        let conn = &mut DB.get_conn().expect("failed to get new connection");

        let loop_out = NewLoopOut {
            state: models::LOOP_OUT_STATE_INITIATED.to_string(),
        };
        let mut invoice = NewInvoice {
            state: models::INVOICE_STATE_OPEN.to_string(),
            payment_hash: "test-payhash",
            payment_preimage: Some("test-preimage"),
            payment_request: "test-invoice",
            amount: 100,
            loop_out_id: 0,
        };
        let mut script = NewScript {
            loop_out_id: 0,
            address: "test-address",
            external_tapkey: "test-external-tapkey",
            internal_tapkey: "test-internal-tapkey",
            internal_tapkey_tweak: "test-internal-tapkey-tweak",
            payment_hash: "test-payment-hash",
            tree: vec!["test-tree".to_string(), "test-tree2".to_string()],
            cltv_expiry: 100,
            remote_pubkey: "test-remote-pubkey".to_string(),
            local_pubkey: "test-local-pubkey".to_string(),
            local_pubkey_index: 100,
        };
        let mut utxo = NewUTXO {
            txid: "test-txid",
            vout: 100,
            amount: 100,
            script_id: 0,
        };
        let resp =
            super::insert_full_loop_out_data(conn, loop_out, &mut invoice, &mut script, &mut utxo);

        assert!(resp.is_ok());

        let full_loop_out = resp.unwrap();

        assert_eq!(
            full_loop_out.loop_out.state,
            models::LOOP_OUT_STATE_INITIATED
        );
        // Assert all db FKs are set correctly
        assert_eq!(
            Some(full_loop_out.loop_out.id),
            full_loop_out.invoice.loop_out_id
        );
        assert_eq!(
            Some(full_loop_out.loop_out.id),
            full_loop_out.script.loop_out_id
        );
        assert_eq!(full_loop_out.script.id, full_loop_out.utxo.script_id);

        assert_new_invoice_matches_invoice(invoice, full_loop_out.invoice);
        assert_new_script_matches_script(script, full_loop_out.script);
        assert_new_utxo_matches_utxo(utxo, full_loop_out.utxo);
    }

    fn assert_new_invoice_matches_invoice(ni: NewInvoice, si: Invoice) {
        assert_eq!(ni.state, si.state);
        assert_eq!(ni.payment_request, si.payment_request);
        assert_eq!(ni.payment_hash, si.payment_hash);
        match (ni.payment_preimage, si.payment_preimage) {
            (Some(ni_preimage), Some(si_preimage)) => assert_eq!(ni_preimage, si_preimage),
            _ => assert!(false),
        }
        assert_eq!(ni.amount, si.amount);
    }

    fn assert_new_script_matches_script(ns: NewScript, ss: Script) {
        assert_eq!(ns.address, ss.address);
        assert_eq!(ns.external_tapkey, ss.external_tapkey);
        assert_eq!(ns.internal_tapkey, ss.internal_tapkey);
        assert_eq!(ns.internal_tapkey_tweak, ss.internal_tapkey_tweak);
        assert_eq!(ns.payment_hash, ss.payment_hash);
        assert_eq!(ns.cltv_expiry, ss.cltv_expiry);
        assert_eq!(ns.remote_pubkey, ss.remote_pubkey);
        assert_eq!(ns.local_pubkey, ss.local_pubkey);
        assert_eq!(ns.local_pubkey_index, ss.local_pubkey_index);
        for (nsi, ssi) in ns.tree.iter().zip(ss.tree.iter()) {
            match ssi {
                Some(ssi) => assert_eq!(*nsi, *ssi),
                None => assert!(false),
            }
        }
    }

    fn assert_new_utxo_matches_utxo(nu: NewUTXO, su: Utxo) {
        assert_eq!(nu.txid, su.txid);
        assert_eq!(nu.vout, su.vout);
        assert_eq!(nu.amount, su.amount);
    }
}
