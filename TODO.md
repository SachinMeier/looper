API -> LN

API -> DB


FIX Proto generation and stop using tonic_lnd

LOOP OUT

1. Buyer requests Loop out from Seller, provides pubkey
    - pubkey B
2. Seller creates invoice for Loop out, creates pubkey, creates script ( B + Preimage | S + timeout)
    - pubkey S
    - tweak r
    - taproot SpendInfo (optional but helpful)
        - external key
        - Internal Key
        - Tree = {B + Preimage, S + timeout}
    - invoice 
        TODO - optional init invoice with amount ~= onchain_fee + loop_fee
3. Buyer waits until output to script is confirmed
4. Buyer pays invoice, receives preimage
TODO 5. Optionally Buyer requests cooperation to move funds to a new address. Otherwise spends B+preimage

API: 
- /looper/out
    - POST
        - pubkey
        - amount
- /looper/out/{id}
    - GET

loop_out
- id
- state = {requested, onchain, complete, cancelled}
- buyer_pubkey
- looper_pubkey
- looper_pubkey_index
- CLTV timeout
- invoice_id
- utxo_id

invoice
- payment_hash
- preimage
- amount
- expiry
- state = {OPEN, SETTLED, CANCELLED}

utxo 
- txid
- vout
- amount
- address
