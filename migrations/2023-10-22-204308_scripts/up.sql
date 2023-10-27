-- Your SQL goes here
CREATE TABLE IF NOT EXISTS scripts (
    id                  BIGSERIAL   PRIMARY KEY,
    loop_out_id         BIGINT      REFERENCES loop_outs(id) ON DELETE CASCADE,
    address             TEXT        NOT NULL,
    external_tapkey        TEXT        NOT NULL,
    internal_tapkey        TEXT        NOT NULL,
    internal_tapkey_tweak  TEXT        NOT NULL,
    tree                TEXT[]      NOT NULL,
    cltv_expiry         INT         NOT NULL,
    remote_pubkey       TEXT        NOT NULL,
    local_pubkey        TEXT        NOT NULL,
    local_pubkey_index  INT         NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX IF NOT EXISTS scripts_address_idx ON scripts(address);