use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
};

use bdk::bitcoin::secp256k1::rand::{self, RngCore};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio::sync::{
    mpsc::{self, Receiver, Sender},
    oneshot, Mutex,
};
use tokio_postgres::{Error, NoTls};

use crate::settings;

mod embedded {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

lazy_static::lazy_static! {
    static ref DB: Db = Db::new();
}

macro_rules! db_call {
    ($op:expr, $($matcher:pat => $result:expr),*) => {
        let (tx, rx) = oneshot::channel();
        DB.tx.blocking_send(($op, tx)).unwrap();
        match rx.blocking_recv().unwrap() {
            $($matcher => $result,)*
            DbResult::Error { error } => Err(error),
            _ => panic!(),
        }
    };
}

pub struct Db {
    is_started: Arc<AtomicBool>,
    pg_config: tokio_postgres::Config,
    pool: Pool,
    rx: Mutex<Receiver<(DbOp, oneshot::Sender<DbResult>)>>,
    stop_thread: Arc<AtomicBool>,
    tx: Sender<(DbOp, oneshot::Sender<DbResult>)>,
}

impl Db {
    pub fn new() -> Self {
        let cfg = settings::build_config().expect("failed to build config");
        
        let host = cfg.get_string("db.host").unwrap();
        let port = cfg.get_int("db.port").unwrap();
        let user = cfg.get_string("db.user").unwrap();
        let pass = cfg.get_string("db.pass").unwrap();
        let name = cfg.get_string("db.name").unwrap();

        let mut pg_config = tokio_postgres::Config::new();
        pg_config.host(&host);
        pg_config.port(port as u16);
        pg_config.user(&user);
        pg_config.password(pass.to_string());
        pg_config.dbname(&name);
        let mgr_config = ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        };
        let mgr = Manager::from_config(pg_config.clone(), NoTls, mgr_config);
        let pool = Pool::builder(mgr).max_size(16).build().unwrap();

        let (tx, rx) = mpsc::channel::<(DbOp, oneshot::Sender<DbResult>)>(16);
        
        return Self {
            is_started: Arc::new(AtomicBool::new(false)),
            pg_config,
            pool,
            rx: Mutex::new(rx),
            stop_thread: Arc::new(AtomicBool::new(false)),
            tx,
        }
    }

    pub fn start() {
        if DB.is_started.load(Ordering::Acquire) {
            return;
        }
        let is_started_loop = DB.is_started.clone();
        let stop_thread_loop = DB.stop_thread.clone();

        thread::Builder::new()
            .name("db".to_string())
            .spawn(move || {
                log::info!("starting db thread");
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap();
                runtime.block_on(async move {
                    loop {
                        is_started_loop.store(true, Ordering::Release);
                        if stop_thread_loop.load(Ordering::Acquire) {
                            log::info!("stopping db thread");
                            return;
                        }

                        let (op, res_tx) = DB.rx.lock().await.recv().await.unwrap();
                        Db::handle_op(op, res_tx).await;
                    }
                });
            })
            .unwrap();
    }

    async fn handle_op(op: DbOp, res_tx: oneshot::Sender<DbResult>) {
        // let client = DB.pool.get().await.unwrap();
        let res = match op {
            DbOp::Migrate => {
                log::info!("starting migrations");
                let (mut migrate_client, migrate_conn) = DB.pg_config.connect(NoTls).await.unwrap();
                tokio::spawn(async move {
                    migrate_conn.await.unwrap();
                });
                embedded::migrations::runner()
                    .run_async(&mut migrate_client)
                    .await
                    .unwrap();
                DbResult::Empty
            }
        };
        res_tx.send(res).unwrap();
    }

    pub fn stop() {
        DB.stop_thread.store(true, Ordering::Release);
    }

    pub fn migrate() -> Result<(), Error> {
        db_call! { DbOp::Migrate, DbResult::Empty => Ok(()) }
    }

}


#[derive(Debug)]
enum DbOp {
    Migrate,
}

#[derive(Debug)]
enum DbResult {
    Empty,
    Error { error: Error },
}

#[cfg(test)]
pub mod tests {
    use std::sync::Once;

    use bdk::bitcoin::{
        secp256k1::{
            rand::{self, RngCore},
            Secp256k1,
        },
        util::bip32::{ExtendedPrivKey, ExtendedPubKey},
        Network,
    };

    use crate::settings;

    use super::{Db};

    static INIT: Once = Once::new();

    pub fn setup_test_db() {
        INIT.call_once(|| {
            // settings::init_logging();
            Db::start();
            Db::migrate().unwrap();
            // Db::truncate().unwrap();
        });
    }

  
}
