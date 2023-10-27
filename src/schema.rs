// @generated automatically by Diesel CLI.

diesel::table! {
    invoices (id) {
        id -> Int8,
        loop_out_id -> Nullable<Int8>,
        payment_request -> Text,
        payment_hash -> Text,
        payment_preimage -> Nullable<Text>,
        amount -> Int8,
        state -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    loop_outs (id) {
        id -> Int8,
        state -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    scripts (id) {
        id -> Int8,
        loop_out_id -> Nullable<Int8>,
        address -> Text,
        external_tapkey -> Text,
        internal_tapkey -> Text,
        internal_tapkey_tweak -> Text,
        tree -> Array<Nullable<Text>>,
        cltv_expiry -> Int4,
        remote_pubkey -> Text,
        local_pubkey -> Text,
        local_pubkey_index -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    utxos (id) {
        id -> Int8,
        txid -> Text,
        vout -> Int4,
        amount -> Int8,
        script_id -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(invoices -> loop_outs (loop_out_id));
diesel::joinable!(scripts -> loop_outs (loop_out_id));
diesel::joinable!(utxos -> scripts (script_id));

diesel::allow_tables_to_appear_in_same_query!(
    invoices,
    loop_outs,
    scripts,
    utxos,
);
