use postgres::{Client, Error};

const CREATE_INVOICES_TABLE_QUERY: &str = "
CREATE TABLE IF NOT EXISTS invoices (
    id                  BIGSERIAL   PRIMARY KEY,
    payment_request     TEXT        NOT NULL,
    payment_hash        TEXT        NOT NULL,
    payment_preimage    TEXT        NOT NULL,
    amount              BIGINT      NOT NULL,
    state               TEXT        NOT NULL,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);";

const CREATE_UTXOS_TABLE_QUERY: &str = "
CREATE TABLE IF NOT EXISTS utxos (
    id                 BIGSERIAL   PRIMARY KEY,
    txid               TEXT        NOT NULL,
    vout               INT         NOT NULL,
    amount             BIGINT      NOT NULL,
    address            TEXT        NOT NULL,
    created_at         TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMP   NOT NULL DEFAULT NOW()
);";

const CREATE_LOOP_OUTS_TABLE_QUERY: &str = "
CREATE TABLE IF NOT EXISTS loop_outs (
    id                  BIGSERIAL   PRIMARY KEY,
    state               TEXT        NOT NULL,
    buyer_pubkey        TEXT        NOT NULL,
    seller_pubkey       TEXT        NOT NULL,
    cltv_timeout        BIGINT      NOT NULL,
    invoice_id          BIGINT      NOT NULL REFERENCES invoices(id),
    utxo_id             BIGINT      NOT NULL REFERENCES utxos(id),
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);";


pub fn migrate(client: &mut Client) -> Result<(), Error> {
    let migrations: Vec<&str> = vec![
        CREATE_INVOICES_TABLE_QUERY,
        CREATE_UTXOS_TABLE_QUERY,
        CREATE_LOOP_OUTS_TABLE_QUERY,
    ];

    for migration in migrations {
        client.batch_execute(migration)?;
    }
    return Ok(());
}