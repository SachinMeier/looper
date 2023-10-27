use crate::schema::{invoices, loop_outs, scripts, utxos};
use diesel::deserialize::FromSql;
use diesel::pg::sql_types::Jsonb;
use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::serialize::{Output, ToSql};
use diesel::sql_types::Array;
use diesel::{AsExpression, FromSqlRow};

// pub enum InvoiceState {
//     Open,
//     Settled,
//     Cancelled,
// }

#[derive(Insertable)]
#[diesel(belongs_to(LoopOut))]
#[table_name = "invoices"]
pub struct NewInvoice<'a> {
    pub loop_out_id: i64,
    pub payment_request: &'a str,
    pub payment_hash: &'a str,
    pub payment_preimage: Option<&'a str>,
    pub amount: i64,
    pub state: String,
}

#[derive(Debug, Queryable, AsChangeset)]
// #[table_name = "invoices"]
pub struct Invoice {
    pub id: i64,
    pub loop_out_id: Option<i64>,
    pub payment_request: String,
    pub payment_hash: String,
    pub payment_preimage: Option<String>,
    pub amount: i64,
    pub state: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// Scripts

#[derive(Insertable)]
#[diesel(belongs_to(LoopOut))]
#[table_name = "scripts"]
pub struct NewScript<'a> {
    pub loop_out_id: i64,
    pub address: &'a str,
    pub external_tapkey: &'a str,
    pub internal_tapkey: &'a str,
    pub internal_tapkey_tweak: &'a str,
    pub tree: Vec<String>,
    pub cltv_expiry: i32,
    pub remote_pubkey: String,
    pub local_pubkey: String,
    pub local_pubkey_index: i32,
}

#[derive(Debug, Queryable, AsChangeset)]
#[diesel(belongs_to(LoopOut))]
pub struct Script {
    pub id: i64,
    pub loop_out_id: Option<i64>,
    pub address: String,
    pub external_tapkey: String,
    pub internal_tapkey: String,
    pub internal_tapkey_tweak: String,
    // TODO: replace tree with payment_hash
    pub tree: Vec<Option<String>>,
    pub cltv_expiry: i32,
    pub remote_pubkey: String,
    pub local_pubkey: String,
    pub local_pubkey_index: i32,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// UTXOs

#[derive(Insertable, Associations)]
#[diesel(belongs_to(Script))]
#[table_name = "utxos"]
pub struct NewUTXO<'a> {
    pub txid: &'a str,
    pub vout: i32,
    pub amount: i64,
    pub script_id: i64,
}

#[derive(Debug, Queryable, Associations, AsChangeset)]
#[diesel(belongs_to(Script))]
#[table_name = "utxos"]
pub struct UTXO {
    pub id: i64,
    pub txid: String,
    pub vout: i32,
    pub amount: i64,
    pub script_id: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// Loop Outs

#[derive(Insertable)]
#[table_name = "loop_outs"]
pub struct NewLoopOut {
    pub state: String,
}

#[derive(Debug, Queryable, AsChangeset)]
#[table_name = "loop_outs"]
pub struct LoopOut {
    pub id: i64,
    pub state: String,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Debug)]
pub struct FullLoopOutData {
    pub loop_out: LoopOut,
    pub script: Script,
    pub utxos: Vec<UTXO>,
    pub invoice: Invoice,
}
