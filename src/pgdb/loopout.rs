// use postgres::{Client, Error, NoTls};
// use postgres::types::Type;
// use chrono::NaiveDateTime;
// // use crate::invoice::Invoice;

// #[derive(Debug)]
// pub struct LoopOut {
//     pub id: i64,
//     pub state: String,
//     pub buyer_pubkey: String,
//     pub seller_pubkey: String,
//     pub cltv_timeout: i64,
//     pub invoice: *mut Invoice,
//     pub created_at: NaiveDateTime,
//     pub updated_at: NaiveDateTime,
// }