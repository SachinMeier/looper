# Looper

Looper is a rust implementation of Lightning Loop functionality. It allows users to move funds between the onchian Bitcoin and Lightning network.

## Disclaimer

!! This is a work in progress and should not be used in production. Only use with funds you are willing to lose. !!

## Contributing

On that note, if you are interested in contributing, take a look at the issues and feel free to assign one to yourself or open a PR. I'm happy to help anyone get started. 

## Running Looper Server Locally

### Requirements

- A running postgres instance.
- A running bitcoind instance.
- A running lnd instance.

For the second 2, bitcoind and lnd, I suggest using [Polar](https://lightningpolar.com/). It's a simple GUI tool allowing you to launch a number of lnd and bitcoind instances. Polar also has cLightning and Eclair support, but Looper is not yet built to integrate with those. That is definitely a goal for the future. 

### Setup

1. Create a `config/default.toml` file and fill in the values based on the `config/example.toml` file.

2. Create a `.env` file and add an extended private key (`xprv`) to it. See `.env.sample` for an example. You can generate one at [bip32.org](https://bip32.org/) by clicking "Bitcoin Mainnet" in the top right corner and switching to "Bitcoin Testnet". I only suggest using this website for testnet keys.

3. Run `source .env`

4. Run cargo run to start the server.

```bash
cargo run
```

The migrations should run and the server should be available at `localhost:8080`.

## Looper Client

Currently, only the server is in a working state. The client (code at `/client`) is not yet functional. However, you can use Postman or curl to interact with the server once it is running by submitting a JSON `POST` request to `localhost:8080`. Note that, for now, the server accepts an x-only pubkey (without the `02` or `03` prefix). This may change in the future.

```json
{
    "amount": 100000,
    "pubkey": "<32-byte X-only pubkey>"
}
```

## Flow

This is how a LoopOut flow works.

1. Buyer/Client requests Loop out from Seller/Server, provides pubkey `B` and an amount.
    - pubkey B
    - Amount
2. Seller creates invoice for the LoopOut with the value of `amount + fee`, creates pubkey `S`, creates Taproot tree with two scripts:
    `B + Preimage` and `S + CLTV timeout`. Seller returns the following data:
    - pubkey S
    - tweak r
    - taproot SpendInfo
        - external key
        - Internal Key
        - Tree = {B + Preimage, S + timeout}
    - invoice 
    - CLTV timeout for the UTXO
    FUTURE - optional init invoice with amount ~= onchain_fee + loop_fee
3. Buyer waits until output to script is confirmed. He must also verify the following
    - He can fully reconstruct the Taproot output script from the data provided by the Seller and his own pubkey `B`. This verifies that the correct pubkeys were used, that the payment hash and pubkey `B` allow him to spend the UTXO, and that the CLTV timeout is correct.
    - The payment hash in the invoice matches the hash in the Taproot output script.
    - The Internal Tapkey is provably unspendable.
    - The CLTV timeout is more than the invoice's minimum CLTV delta blocks in the future, optimistically showing that the Seller cannot hold on to the invoice long enough to potentially claim the LN payment  and steal the UTXO via timeout script. 
4. Buyer pays invoice, receives preimage
5. Buyer claims UTXO onchain
FUTURE 5. Optionally Buyer requests cooperation to move funds to a new address using MuSig2 in the Internal key. Otherwise spends B+preimage
