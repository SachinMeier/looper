-- Your SQL goes here
CREATE TABLE IF NOT EXISTS loop_outs (
    id                  BIGSERIAL   PRIMARY KEY,
    state               TEXT        NOT NULL,
    created_at          TIMESTAMP   NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMP   NOT NULL DEFAULT NOW()
);
