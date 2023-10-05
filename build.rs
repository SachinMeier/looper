use std::path::PathBuf;

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=proto");
    let dir = PathBuf::from("proto/lnd");

    let protos = [
        "lightning.proto",
        "invoicesrpc/invoices.proto",
        "routerrpc/routerrpc.proto",
        "walletrpc/walletkit.proto",
        "peersrpc/peers.proto",
        "signrpc/signer.proto",
        "verrpc/verrpc.proto",
    ];

    let proto_paths: Vec<_> = protos
        .iter()
        .map(|proto| {
            let path = dir.join(proto);
            path
        })
        .collect();
    
    tonic_build::configure()
        .build_client(true)
        .build_server(false)
        .format(false)
        .compile(&proto_paths, &[dir])?;

    Ok(())
}
