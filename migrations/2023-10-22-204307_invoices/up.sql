-- Your SQL goes here
CREATE TABLE IF NOT EXISTS invoices (
    id                  UUID   PRIMARY KEY,
    loop_out_id         UUID      REFERENCES loop_outs(id) ON DELETE CASCADE,
    payment_request     TEXT        NOT NULL,
    payment_hash        TEXT        NOT NULL,
    payment_preimage    TEXT,
    amount              BIGINT      NOT NULL,
    state               TEXT        NOT NULL,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);
