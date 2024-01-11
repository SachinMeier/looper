-- Your SQL goes here
CREATE TABLE IF NOT EXISTS utxos (
    id                 BIGSERIAL   PRIMARY KEY,
    txid               TEXT        NOT NULL,
    vout               INT         NOT NULL,
    amount             BIGINT      NOT NULL,
    script_id          BIGINT      NOT NULL REFERENCES scripts(id),
    created_at         TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at         TIMESTAMP   NOT NULL DEFAULT NOW()
);
