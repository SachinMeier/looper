# Looper

Looper will do 2 things: `LoopOuts` and `LoopIns`, just like Lightning Labs' Loop.

LOOP OUT

1. Buyer requests Loop out from Seller, provides pubkey
    - pubkey B
    - Amount
2. Seller creates invoice for Loop out, creates pubkey, creates script ( B + Preimage | S + timeout)
    - pubkey S
    - tweak r
    - taproot SpendInfo (optional but helpful)
        - external key
        - Internal Key
        - Tree = {B + Preimage, S + timeout}
    - invoice 
        FUTURE - optional init invoice with amount ~= onchain_fee + loop_fee
3. Buyer waits until output to script is confirmed
4. Buyer pays invoice, receives preimage
5. Buyer claims UTXO onchain
FUTURE 5. Optionally Buyer requests cooperation to move funds to a new address using MuSig2 in the Internal key. Otherwise spends B+preimage

## Simple Architecture:

This is the current plan for the architecture. It's simple, but i'm new to Rust and unfamiliar with how to allow both
LoopInSvc and LoopOutSvc to manage the same resources. In any case, we aren't building LoopInSvc yet. 

```
API --> LoopOutSvc --> LNDG
    |              \--> DB
    |               \--> Wallet
    |
    \--> LoopInSvc --> LNDG
                   \--> DB
                    \--> Wallet
```

Schema:

```
loop_out <--- utxo <--- script
    |
    \--- invoice
```

See `migrations/` for the schema of each table.

## Tasks

I would like to build a complete working example (client & Server) with `LoopOut` first, before moving to `LoopIns`

API:
1. Build full API endpoints for each function
    - POST /loop/out - create loop out
    - GET /loop/out/{payment_hash} - get loop out

DB:
1. Finish Diesel DB setup
    - Create Diesel objects for LoopOuts, UTXOs, Invoices
    - Ideally, both client and server can share 1 DB schema, since they will both need the same data. 
2. add read/write functions for each table
3. Add DB functions to LoopOutSvc

Wallet:
1. Refactor, break out functions for cleaner, more modular code. 
2. Get some fee estimation strategy going. Probably bitcoind via RPC for now. Maybe mempool.space is better? 

LoopOutSvc:
1. Get CLTV timeouts working properly. 
    - Invoice payment needs to expire N blocks before the UTXO expires. 40 is minCLTVDelta in LND, so might be sufficient.
    - LND should auto-timeout the payment. Ensure this is true. 
2. Server should be timeout aware and reclaim UTXOs. 

Code Organization: 
1. Factor this repo into:
    - common modules (where should this go?)
    - server
    - client
2. Get rid of all the unwraps and unhandled errors. Setup a good error handling strategy.
3. Get payment_request vs invoice naming as uniform as possible.
4. Comment code.

Client:
1. API Client
    - Create API client for each endpoint
    - Create API client for each endpoint

2. Main
    - Create Client Wallet (reuse server's Wallet pkg)
    - Create LNDGateway (reuse server's LNDGateway pkg)
    - Create Client LoopOutSvc (not using the server's LoopOutSvc pkg)
    - Create API Client

3. LoopOutSvc
    - Create check for CLTV expiry before paying invoice. 