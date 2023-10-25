-- Your SQL goes here
CREATE TABLE IF NOT EXISTS scripts (
    id                  BIGSERIAL   PRIMARY KEY,
    address             TEXT        NOT NULL,
    external_key        TEXT        NOT NULL,
    internal_key        TEXT        NOT NULL,
    internal_key_tweak  TEXT        NOT NULL,
    tree                JSONB       NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS scripts_address_idx ON scripts(address);