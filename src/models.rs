use crate::schema::{invoices, loop_outs, scripts, utxos};
// use diesel::deserialize::FromSql;
// use diesel::pg::sql_types::Jsonb;
// use diesel::pg::Pg;
use diesel::prelude::*;
// use diesel::serialize::{Output, ToSql};
// use diesel::sql_types::Array;
// use diesel::{AsExpression, FromSqlRow};

// pub enum InvoiceState {
//     Open,
//     Settled,
//     Cancelled,
// }

pub const INVOICE_STATE_OPEN: &str = "OPEN";
#[allow(dead_code)]
pub const INVOICE_STATE_SETTLED: &str = "SETTLED";
#[allow(dead_code)]
pub const INVOICE_STATE_CANCELLED: &str = "CANCELLED";

#[derive(Insertable, Clone)]
#[diesel(belongs_to(LoopOut))]
#[diesel(table_name = invoices)]
pub struct NewInvoice<'a> {
    pub loop_out_id: i64,
    pub payment_request: &'a str,
    pub payment_hash: &'a str,
    pub payment_preimage: Option<&'a str>,
    pub amount: i64,
    pub state: String,
}

#[derive(Debug, Queryable, AsChangeset)]
#[diesel(table_name = invoices)]
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

#[derive(Insertable, Clone)]
#[diesel(belongs_to(LoopOut))]
#[diesel(table_name = scripts)]
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
#[diesel(table_name = scripts)]
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

#[derive(Insertable, Associations, Clone)]
#[diesel(belongs_to(Script))]
#[diesel(table_name = utxos)]
pub struct NewUTXO<'a> {
    pub txid: &'a str,
    pub vout: i32,
    pub amount: i64,
    pub script_id: i64,
}

#[derive(Debug, Queryable, Associations, AsChangeset)]
#[diesel(belongs_to(Script))]
#[diesel(table_name = utxos)]
pub struct Utxo {
    pub id: i64,
    pub txid: String,
    pub vout: i32,
    pub amount: i64,
    pub script_id: i64,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: chrono::NaiveDateTime,
}

// Loop Outs

/// LOOP_OUT_STATE_INITIATED should be set when the server has registered the loop out and returned the swap invoice.
pub const LOOP_OUT_STATE_INITIATED: &str = "INITIATED";
/// LOOP_OUT_STATE_CONFIRMED should be set when the funding transaction is confirmed onchain.
#[allow(dead_code)]
pub const LOOP_OUT_STATE_CONFIRMED: &str = "CONFIRMED";
/// LOOP_OUT_STATE_CLAIMED should be set when the server has seen the claim transaction in the mempool or confirmed onchain.
#[allow(dead_code)]
pub const LOOP_OUT_STATE_CLAIMED: &str = "CLAIMED";
/// LOOP_OUT_STATE_TIMEOUT should be set when the server has broadcast the timeout spend back to the server's wallet.
#[allow(dead_code)]
pub const LOOP_OUT_STATE_TIMEOUT: &str = "TIMEOUT";

#[derive(Insertable, Clone)]
#[diesel(table_name = loop_outs)]
pub struct NewLoopOut {
    pub state: String,
}

#[derive(Debug, Queryable, AsChangeset)]
#[diesel(table_name = loop_outs)]
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
    pub utxo: Utxo,
    pub invoice: Invoice,
}
