// @generated automatically by Diesel CLI.

diesel::table! {
    invoices (id) {
        id -> Int8,
        payment_request -> Text,
        payment_hash -> Text,
        payment_preimage -> Text,
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
        buyer_pubkey -> Text,
        looper_pubkey -> Text,
        looper_pubkey_index -> Int4,
        cltv_timeout -> Int8,
        invoice_id -> Int8,
        utxo_id -> Int8,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    refinery_schema_history (version) {
        version -> Int4,
        #[max_length = 255]
        name -> Nullable<Varchar>,
        #[max_length = 255]
        applied_on -> Nullable<Varchar>,
        #[max_length = 255]
        checksum -> Nullable<Varchar>,
    }
}

diesel::table! {
    utxos (id) {
        id -> Int8,
        txid -> Text,
        vout -> Int4,
        amount -> Int8,
        address -> Text,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::joinable!(loop_outs -> invoices (invoice_id));
diesel::joinable!(loop_outs -> utxos (utxo_id));

diesel::allow_tables_to_appear_in_same_query!(
    invoices,
    loop_outs,
    refinery_schema_history,
    utxos,
);
