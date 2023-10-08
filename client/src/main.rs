mod client;
mod services;

use bdk::bitcoin::secp256k1::XOnlyPublicKey;
use looper::{
    lnd::client::LNDGateway,
    services::loop_out::{LoopOutInfo, LoopOutResponse, TaprootScriptInfo},
    settings,
    wallet::LooperWallet,
};
fn main() {
    // setup();
    settle();
}

fn startup() {
    settings::init_logging();

    // Db::start();
    // Db::migrate().unwrap();

    LNDGateway::start();
}

fn setup() {
    let cfg = settings::build_config().unwrap();
    startup();

    let wallet = LooperWallet::new(&cfg);

    let (pubkey, pubkey_idx) = wallet.new_pubkey();
    log::info!("pubkey: {}", hex::encode(pubkey.serialize()));
}

fn settle() {
    let cfg = settings::build_config().unwrap();
    startup();

    let wallet = LooperWallet::new(&cfg);

    let pubkey_idx = 0;
    let amount = 1_000_000;
    let addr_str = "bcrt1pwykdms3qz4hq4qe9hczqs04hrzstcj8w5x7pdmych9cvdf84e25src054u";
    let addr = wallet.validate_address(addr_str).unwrap();

    let loop_out_svc = services::loop_out::LoopOutService::new(wallet);

    let curr_height = loop_out_svc.refresh_wallet();

    let resp = LoopOutResponse {
        invoice: "lnbcrt10m1pjjdfu9pp5eyj56qjxhyhq2lyawz9wzk4vykah9gduapxapkwpsk8su8afq0wsdqcd3hk7ur9wgs8xampwqsx7at5cqzzsxqyznlssp5hha4xxp0lye4pssu9egl3yuf6vh2hj272x06xfepavls79rwtarq9qyyssq9awcl65f3u08z6u5rls9aff9k9evgfgj0p5klgkfk4a5es3zmhgp9wggtw57p22dqkc9d6vckwry6ra9ysa0up7smu7apj0vrq2dqtgpx0f50q".to_string(),
        address: "bcrt1p02gwm45e6mt7dc70m244wzxdpzgmml7uxy8n9re7acwa3lus4a3qyxpe6a".to_string(),
        looper_pubkey: "146846eeb5a7533abb594ba734bc243fc7b6349499b8311c8fc13b0112ba8a77".to_string(),
        txid: "4b3e7c3bec851ec0aff033c3641103bc79927e435fd9f60c503a868b800b58bd".to_string(),
        vout: 0,
        taproot_script_info: TaprootScriptInfo {
            external_key: "5a5155b445b3fffbe78f1c4120c68260329168c74cbdda6210e937366bc62ee1".to_string(),
            internal_key: "acc003b851906c5edbc7cc54ec269690d71d6967ec4947184f218ec1c0ddc7b5".to_string(),
            internal_key_tweak: "866d58a585a99265f0a2ac7d307c573120c5ad775f76f0eb7474a6b2fa307214".to_string(),
            tree: vec!["02570eb17520146846eeb5a7533abb594ba734bc243fc7b6349499b8311c8fc13b0112ba8a77ac".to_string(), 
                "20146846eeb5a7533abb594ba734bc243fc7b6349499b8311c8fc13b0112ba8a77ad82012088a820c9254d0246b92e057c9d708ae15aac25bb72a1bce84dd0d9c1858f0e1fa903dd87".to_string()],
        },
        loop_info: LoopOutInfo {
            fee: 0,
            loop_hash: "c9254d0246b92e057c9d708ae15aac25bb72a1bce84dd0d9c1858f0e1fa903dd".to_string(),
            cltv_expiry: 3671,
        },
        error: None,
    };

    loop_out_svc.handle_loop_out_response(resp, pubkey_idx, addr, amount as u64, curr_height);
}
