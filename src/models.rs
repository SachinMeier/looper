use crate::schema::{invoices, loop_outs, utxos};
use diesel::prelude::*;

// pub enum InvoiceState {
//     Open,
//     Settled,
//     Cancelled,
// }

#[derive(Insertable)]
#[table_name = "invoices"]
pub struct NewInvoice<'a> {
    pub payment_request: &'a str,
    pub payment_hash: &'a str,
    pub payment_preimage: &'a str,
    pub amount: i64,
    pub state: String,
}

#[derive(Debug, Queryable, AsChangeset)]
// #[table_name = "invoices"]
pub struct Invoice {
    pub id: i64,
    pub payment_request: String,
    pub payment_hash: String,
    pub payment_preimage: String,
    pub amount: i64,
    pub state: String,
}
