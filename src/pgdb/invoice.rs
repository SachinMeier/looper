use postgres::{Client, Error, NoTls};
use postgres::types::Type;
use chrono::NaiveDateTime;

#[derive(Clone, Debug)]
pub struct Invoice {
    pub id: i64,
    pub payment_request: String,
    pub payment_hash: String,
    pub payment_preimage: String,
    pub amount: i64,
    pub state: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

const INSERT_INVOICE_QUERY: &str = "
INSERT INTO invoices (
    payment_request,
    payment_hash,
    payment_preimage,
    amount,
    state
) VALUES ($1, $2, $3, $4, $5) 
RETURNING id, created_at, updated_at;
";

pub fn insert_invoice(client: &mut Client, invoice: &mut Invoice) -> Result<(), Error> {
    let row = client.query_one(INSERT_INVOICE_QUERY, &[
        &invoice.payment_request,
        &invoice.payment_hash,
        &invoice.payment_preimage,
        &invoice.amount,
        &invoice.state,
    ])?;

    invoice.id = row.get(0);
    invoice.created_at = row.get(1);
    invoice.updated_at = row.get(2);

    return Ok(());
}

const GET_INVOICE_QUERY: &str = "
SELECT id, payment_request, payment_hash, payment_preimage, amount, state, created_at, updated_at
FROM invoices
WHERE id = $1;
";

pub fn get_invoice(client: &mut Client, id: i64) -> Result<Invoice, Error> {
    let row = client.query_one(GET_INVOICE_QUERY, &[&id])?;

    let invoice = Invoice {
        id: row.get(0),
        payment_request: row.get(1),
        payment_hash: row.get(2),
        payment_preimage: row.get(3),
        amount: row.get(4),
        state: row.get(5),
        created_at: row.get(6),
        updated_at: row.get(7),
    };

    return Ok(invoice);
}

