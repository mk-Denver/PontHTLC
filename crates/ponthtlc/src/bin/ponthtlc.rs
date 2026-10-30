use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use ponthtlc::bitcoind::BitcoindFacade;
use ponthtlc::http::{router, AppState};
use ponthtlc::node::build_node;
use ponthtlc::tenancy::EscrowStore;

#[tokio::main]
async fn main() {
    let cfg = parse_config();
    let bitcoind = Arc::new(BitcoindFacade::new(
        cfg.bitcoind_host,
        cfg.bitcoind_port,
        cfg.rpc_user,
        cfg.rpc_pass,
    ));
    if let Err(err) = bitcoind.wait_for_bitcoind(45).await {
        eprintln!("bitcoind unavailable: {err}");
        std::process::exit(1);
    }
    if let Err(err) = bitcoind.ensure_wallet("spike").await {
        eprintln!("bitcoind wallet unavailable: {err}");
        std::process::exit(1);
    }
    let data_dir = cfg.data_dir.clone();
    let node = build_node("ponthtlc", data_dir.clone(), cfg.ln_port, bitcoind).await;
    let store = match EscrowStore::open(data_dir.join("escrows.json")) {
        Ok(store) => store,
        Err(err) => {
            eprintln!("escrow store unavailable: {err}");
            std::process::exit(1);
        },
    };
    let state = Arc::new(AppState { node, store: Mutex::new(store) });
    let app = router(state);
    let addr = SocketAddr::from(([127, 0, 0, 1], cfg.port));
    let listener = match tokio::net::TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("failed to bind {addr}: {err}");
            std::process::exit(1);
        },
    };
    println!("ponthtlc listening on http://{addr}");
    if let Err(err) = axum::serve(listener, app).await {
        eprintln!("http server stopped: {err}");
        std::process::exit(1);
    }
}

struct DaemonConfig {
    data_dir: std::path::PathBuf,
    port: u16,
    ln_port: u16,
    bitcoind_host: String,
    bitcoind_port: u16,
    rpc_user: String,
    rpc_pass: String,
}

fn parse_config() -> DaemonConfig {
    let mut args: HashMap<String, String> = HashMap::new();
    let argv: Vec<String> = env::args().collect();
    let mut i = 1;
    while i + 1 < argv.len() {
        let key = argv[i].trim_start_matches("--").to_string();
        args.insert(key, argv[i + 1].clone());
        i += 2;
    }
    let data_dir = args.get("data-dir").cloned().unwrap_or_else(|| {
        eprintln!("--data-dir is required");
        std::process::exit(1);
    });
    let port = args.get("port").map(|value| value.parse().expect("--port")).unwrap_or(9800);
    let ln_port =
        args.get("ln-port").map(|value| value.parse().expect("--ln-port")).unwrap_or(9735);
    DaemonConfig {
        data_dir: data_dir.into(),
        port,
        ln_port,
        bitcoind_host: args.get("bitcoind-host").cloned().unwrap_or_else(|| "127.0.0.1".to_string()),
        bitcoind_port: args
            .get("bitcoind-port")
            .map(|value| value.parse().expect("--bitcoind-port"))
            .unwrap_or(18923),
        rpc_user: args.get("rpc-user").cloned().unwrap_or_else(|| {
            eprintln!("warning: using default rpc-user 'ponthtlc'; pass --rpc-user to override");
            "ponthtlc".to_string()
        }),
        rpc_pass: args.get("rpc-pass").cloned().unwrap_or_else(|| {
            eprintln!("warning: using default rpc-pass; pass --rpc-pass to override");
            "ponthtlc".to_string()
        }),
    }
}
