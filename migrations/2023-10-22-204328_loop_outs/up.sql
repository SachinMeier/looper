-- Your SQL goes here
CREATE TABLE IF NOT EXISTS loop_outs (
    id                  BIGSERIAL   PRIMARY KEY,
    state               TEXT        NOT NULL,
    buyer_pubkey        TEXT        NOT NULL,
    looper_pubkey       TEXT        NOT NULL,
    looper_pubkey_index INT         NOT NULL,
    cltv_timeout        BIGINT      NOT NULL,
    invoice_id          BIGINT      NOT NULL REFERENCES invoices(id),
    utxo_id             BIGINT      NOT NULL REFERENCES utxos(id),
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);