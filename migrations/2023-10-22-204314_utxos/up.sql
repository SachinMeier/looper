-- Your SQL goes here
CREATE TABLE IF NOT EXISTS utxos (
    id                 BIGSERIAL   PRIMARY KEY,
    txid               TEXT        NOT NULL,
    vout               INT         NOT NULL,
    amount             BIGINT      NOT NULL,
    address            TEXT        NOT NULL,
    created_at         TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMP   NOT NULL DEFAULT NOW()
);