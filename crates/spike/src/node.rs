use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use lightning::chain::chainmonitor;
use lightning::chain::Filter;
use lightning::events::Event;
use lightning::ln::channelmanager::{ChainParameters, SimpleArcChannelManager};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, PeerManager};
use lightning::onion_message::messenger::DefaultMessageRouter;
use lightning::routing::gossip::NetworkGraph;
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringDecayParameters, ProbabilisticScoringFeeParameters,
};
use lightning::sign::{InMemorySigner, KeysManager, NodeSigner};
use lightning::util::config::{ChannelHandshakeConfig, ChannelHandshakeLimits, UserConfig};
use lightning::util::logger::{Level, Logger, Record};
use lightning::util::persist::{KVStoreSyncWrapper, MonitorUpdatingPersisterAsync};
use lightning_block_sync::gossip::TokioSpawner;
use lightning_block_sync::poll::ChainPoller;
use lightning_block_sync::{init, SpvClient, UnboundedCache};
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::fs_store::FilesystemStore;
use rand::RngCore;

use crate::bitcoind_proxy::BitcoindFacade;

pub type EventSink = Arc<Mutex<Vec<Event>>>;

pub type ChainMonitorT = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<BitcoindFacade>,
    Arc<BitcoindFacade>,
    Arc<StdLogger>,
    chainmonitor::AsyncPersister<
        KVStoreSyncWrapper<Arc<FilesystemStore>>,
        TokioSpawner,
        Arc<StdLogger>,
        Arc<KeysManager>,
        Arc<KeysManager>,
        Arc<BitcoindFacade>,
        Arc<BitcoindFacade>,
    >,
    Arc<KeysManager>,
>;

pub type ChannelManagerT =
    SimpleArcChannelManager<ChainMonitorT, BitcoindFacade, BitcoindFacade, StdLogger>;

pub type PeerManagerT = PeerManager<
    SocketDescriptor,
    Arc<ChannelManagerT>,
    IgnoringMessageHandler,
    IgnoringMessageHandler,
    Arc<StdLogger>,
    IgnoringMessageHandler,
    Arc<KeysManager>,
    Arc<ChainMonitorT>,
>;

pub struct StdLogger {
    pub name: String,
    pub level: Level,
}

impl Logger for StdLogger {
    fn log(&self, record: Record) {
        if record.level <= self.level && record.level != Level::Gossip {
            println!("[{} {:?}] {}", self.name, record.level, record.args);
        }
    }
}

pub struct Node {
    pub name: &'static str,
    pub node_pubkey: bitcoin::secp256k1::PublicKey,
    pub listen_addr: SocketAddr,
    pub logger: Arc<StdLogger>,
    pub keys_manager: Arc<KeysManager>,
    pub chain_monitor: Arc<ChainMonitorT>,
    pub channel_manager: Arc<ChannelManagerT>,
    pub peer_manager: Arc<PeerManagerT>,
    pub events: EventSink,
    pub bitcoind: Arc<BitcoindFacade>,
}

pub async fn build_node(
    name: &'static str, data_dir: PathBuf, listen_port: u16, bitcoind: Arc<BitcoindFacade>,
) -> Arc<Node> {
    let logger = Arc::new(StdLogger { name: name.to_string(), level: Level::Debug });

    let seed_path = data_dir.join("keys_seed");
    let seed = match std::fs::read(&seed_path) {
        Ok(existing_seed) if existing_seed.len() == 32 => {
            let mut seed_bytes = [0u8; 32];
            seed_bytes.copy_from_slice(&existing_seed);
            seed_bytes
        },
        _ => {
            let mut seed_bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut seed_bytes);
            let _ = std::fs::create_dir_all(&data_dir);
            let _ = std::fs::write(&seed_path, &seed_bytes);
            seed_bytes
        },
    };

    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock to be valid");
    let keys_manager =
        Arc::new(KeysManager::new(&seed, now.as_secs(), now.subsec_nanos(), true));

    let fs_store = KVStoreSyncWrapper(Arc::new(FilesystemStore::new(data_dir.clone())));
    let persister = MonitorUpdatingPersisterAsync::new(
        fs_store,
        TokioSpawner,
        Arc::clone(&logger),
        1000,
        Arc::clone(&keys_manager),
        Arc::clone(&keys_manager),
        Arc::clone(&bitcoind),
        Arc::clone(&bitcoind),
    );

    let chain_monitor = Arc::new(chainmonitor::ChainMonitor::new_async_beta(
        None,
        Arc::clone(&bitcoind),
        Arc::clone(&logger),
        Arc::clone(&bitcoind),
        persister,
        Arc::clone(&keys_manager),
        keys_manager.get_peer_storage_key(),
    ));

    let chain_tip = init::validate_best_block_header(bitcoind.as_ref())
        .await
        .expect("failed to fetch best block header from bitcoind");

    let network_graph = Arc::new(NetworkGraph::new(bitcoin::Network::Regtest, Arc::clone(&logger)));

    let scorer = Arc::new(RwLock::new(ProbabilisticScorer::new(
        ProbabilisticScoringDecayParameters::default(),
        Arc::clone(&network_graph),
        Arc::clone(&logger),
    )));

    let router = Arc::new(DefaultRouter::new(
        Arc::clone(&network_graph),
        Arc::clone(&logger),
        Arc::clone(&keys_manager),
        Arc::clone(&scorer),
        ProbabilisticScoringFeeParameters::default(),
    ));

    let message_router = Arc::new(DefaultMessageRouter::new(
        Arc::clone(&network_graph),
        Arc::clone(&keys_manager),
    ));

    let user_config = UserConfig {
        channel_handshake_limits: ChannelHandshakeLimits {
            their_to_self_delay: 2016,
            ..Default::default()
        },
        channel_handshake_config: ChannelHandshakeConfig {
            minimum_depth: 1,
            announce_for_forwarding: false,
            negotiate_anchors_zero_fee_htlc_tx: false,
            ..Default::default()
        },
        ..Default::default()
    };

    let chain_params = ChainParameters {
        network: bitcoin::Network::Regtest,
        best_block: chain_tip.to_best_block(),
    };

    let channel_manager = Arc::new(ChannelManagerT::new(
        Arc::clone(&bitcoind),
        Arc::clone(&chain_monitor),
        Arc::clone(&bitcoind),
        router,
        Arc::clone(&message_router),
        Arc::clone(&logger),
        Arc::clone(&keys_manager),
        Arc::clone(&keys_manager),
        Arc::clone(&keys_manager),
        user_config,
        chain_params,
        now.as_secs() as u32,
    ));

    let mut ephemeral_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut ephemeral_bytes);
    let current_time: u32 = now.as_secs() as u32;

    let lightning_msg_handler = MessageHandler {
        chan_handler: Arc::clone(&channel_manager),
        route_handler: IgnoringMessageHandler {},
        onion_message_handler: IgnoringMessageHandler {},
        custom_message_handler: IgnoringMessageHandler {},
        send_only_message_handler: Arc::clone(&chain_monitor),
    };

    let peer_manager = Arc::new(PeerManagerT::new(
        lightning_msg_handler,
        current_time,
        &ephemeral_bytes,
        Arc::clone(&logger),
        Arc::clone(&keys_manager),
    ));

    let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, listen_port));
    start_listening(Arc::clone(&peer_manager), listen_port);
    start_chain_sync(
        Arc::clone(&bitcoind),
        chain_tip,
        Arc::clone(&chain_monitor),
        Arc::clone(&channel_manager),
        bitcoin::Network::Regtest,
    );

    let node_pubkey = channel_manager.get_our_node_id();
    let node = Arc::new(Node {
        name,
        listen_addr,
        node_pubkey,
        logger,
        keys_manager,
        chain_monitor,
        channel_manager,
        peer_manager,
        events: Arc::new(Mutex::new(Vec::new())),
        bitcoind,
    });
    println!("SPIKE_INFO: built node {}", name);
    node
}

fn start_listening(peer_manager: Arc<PeerManagerT>, listen_port: u16) {
    let peer_manager_for_accept = Arc::clone(&peer_manager);
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", listen_port))
            .await
            .expect("failed to bind spike LDK listen port");
        loop {
            let peer_manager = Arc::clone(&peer_manager_for_accept);
            let tcp_stream = listener.accept().await.expect("accept failed").0;
            tokio::spawn(async move {
                lightning_net_tokio::setup_inbound(peer_manager, tcp_stream.into_std().unwrap())
                    .await;
            });
        }
    });
}

fn start_chain_sync(
    bitcoind: Arc<BitcoindFacade>, chain_tip: lightning_block_sync::poll::ValidatedBlockHeader,
    chain_monitor: Arc<ChainMonitorT>, channel_manager: Arc<ChannelManagerT>,
    network: bitcoin::Network,
) {
    tokio::spawn(async move {
        let chain_poller = ChainPoller::new(bitcoind.as_ref(), network);
        let mut cache = UnboundedCache::new();
        let chain_listener = (chain_monitor, channel_manager);
        let mut spv_client = SpvClient::new(chain_tip, chain_poller, &mut cache, &chain_listener);
        loop {
            spv_client.poll_best_tip().await.unwrap();
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    });
}
