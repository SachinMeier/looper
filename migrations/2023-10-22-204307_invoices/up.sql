-- Your SQL goes here
CREATE TABLE IF NOT EXISTS invoices (
    id                  BIGSERIAL   PRIMARY KEY,
    payment_request     TEXT        NOT NULL,
    payment_hash        TEXT        NOT NULL,
    payment_preimage    TEXT        NOT NULL,
    amount              BIGINT      NOT NULL,
    state               TEXT        NOT NULL,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);