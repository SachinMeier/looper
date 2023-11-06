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