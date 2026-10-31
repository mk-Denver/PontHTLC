use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bitcoin::secp256k1::PublicKey;
use lightning::chain::chainmonitor;
use lightning::chain::{Filter, Watch};
use lightning::events::Event;
use lightning::ln::channelmanager::{
    ChainParameters, ChannelManagerReadArgs, SimpleArcChannelManager,
};
use lightning::ln::peer_handler::{IgnoringMessageHandler, MessageHandler, PeerManager};
use lightning::onion_message::messenger::{DefaultMessageRouter, OnionMessenger};
use lightning::routing::gossip::{NetworkGraph, P2PGossipSync};
use lightning::routing::router::DefaultRouter;
use lightning::routing::scoring::{
    ProbabilisticScorer, ProbabilisticScoringDecayParameters, ProbabilisticScoringFeeParameters,
    Score,
};
use lightning::routing::utxo::UtxoLookup;
use lightning::sign::{ChangeDestinationSourceSync, InMemorySigner, KeysManager, NodeSigner};
use lightning::util::config::{ChannelHandshakeConfig, ChannelHandshakeLimits, UserConfig};
use lightning::util::logger::{Level, Logger, Record};
use lightning::util::persist::{
    KVStoreSync, KVStoreSyncWrapper, MonitorUpdatingPersisterAsync,
    CHANNEL_MANAGER_PERSISTENCE_KEY, CHANNEL_MANAGER_PERSISTENCE_PRIMARY_NAMESPACE,
    CHANNEL_MANAGER_PERSISTENCE_SECONDARY_NAMESPACE,
};
use lightning::util::ser::{ReadableArgs, Writeable, Writer};
use lightning::util::sweep::OutputSweeperSync;
use lightning_background_processor::{BackgroundProcessor, GossipSync};
use lightning_liquidity::utils::time::DefaultTimeProvider;
use lightning_liquidity::LiquidityManagerSync;
use lightning_rapid_gossip_sync::RapidGossipSync;
use lightning::util::native_async::FutureSpawner;
use lightning_block_sync::poll::{ChainPoller, ValidatedBlockHeader};
use lightning_block_sync::{init, SpvClient, UnboundedCache};
use lightning_net_tokio::SocketDescriptor;
use lightning_persister::fs_store::FilesystemStore;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, watch};
use tokio::task::JoinHandle;

use crate::bitcoind::BitcoindFacade;
use crate::events::{event_channel, NodeEventHandler, WireEvent};

pub type EventSink = Arc<Mutex<Vec<Event>>>;

pub type ChainMonitorT = chainmonitor::ChainMonitor<
    InMemorySigner,
    Arc<dyn Filter + Send + Sync>,
    Arc<BitcoindFacade>,
    Arc<BitcoindFacade>,
    Arc<StdLogger>,
    chainmonitor::AsyncPersister<
        KVStoreSyncWrapper<Arc<FilesystemStore>>,
        RuntimeSpawner,
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
        if record.level >= self.level {
            println!("[{} {:?}] {}", self.name, record.level, record.args);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct PeerRecord {
    pubkey: String,
    addr: String,
}

#[derive(Clone, Debug)]
pub struct ChannelSnapshot {
    pub channel_id: String,
    pub ready: bool,
    pub usable: bool,
    pub counterparty: String,
}

pub struct Node {
    pub name: &'static str,
    pub node_pubkey: PublicKey,
    pub listen_addr: SocketAddr,
    pub logger: Arc<StdLogger>,
    pub keys_manager: Arc<KeysManager>,
    pub chain_monitor: Arc<ChainMonitorT>,
    pub channel_manager: Arc<ChannelManagerT>,
    pub peer_manager: Arc<PeerManagerT>,
    pub events: EventSink,
    pub bitcoind: Arc<BitcoindFacade>,
    pub events_tx: broadcast::Sender<WireEvent>,
    data_dir: PathBuf,
    shutdown_tx: watch::Sender<bool>,
    listener: Mutex<Option<JoinHandle<()>>>,
    chain_sync: Mutex<Option<JoinHandle<()>>>,
    background: Mutex<Option<BackgroundProcessor>>,
}

impl Node {
    pub fn subscribe(&self) -> broadcast::Receiver<WireEvent> {
        self.events_tx.subscribe()
    }

    pub fn is_connected_to(&self, pubkey: &PublicKey) -> bool {
        self.peer_manager.peer_by_node_id(pubkey).is_some()
    }

    pub fn listed_channel_ids(&self) -> Vec<String> {
        self.channel_manager
            .list_channels()
            .into_iter()
            .map(|channel| channel.channel_id.to_string())
            .collect()
    }

    pub fn channel_snapshots(&self) -> Vec<ChannelSnapshot> {
        self.channel_manager
            .list_channels()
            .into_iter()
            .map(|channel| ChannelSnapshot {
                channel_id: channel.channel_id.to_string(),
                ready: channel.is_channel_ready,
                usable: channel.is_usable,
                counterparty: channel.counterparty.node_id.to_string(),
            })
            .collect()
    }

    pub async fn wait_for_event<F>(&self, mut predicate: F, timeout_secs: u64) -> Option<Event>
    where
        F: FnMut(&Event) -> bool,
    {
        let deadline = std::time::Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            {
                let sink_lock = self.events.lock().expect("event sink mutex poisoned");
                for event in sink_lock.iter() {
                    if predicate(event) {
                        return Some(event.clone());
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    pub async fn remember_peer(&self, pubkey: PublicKey, addr: SocketAddr) -> Result<(), String> {
        let mut peers = read_peers(&self.data_dir);
        let record = PeerRecord { pubkey: pubkey.to_string(), addr: addr.to_string() };
        if !peers.iter().any(|peer| peer.pubkey == record.pubkey && peer.addr == record.addr) {
            peers.push(record);
            write_peers(&self.data_dir, &peers)?;
        }
        connect_peer(Arc::clone(&self.peer_manager), pubkey, addr).await
    }

    pub async fn shutdown(&self) {
        self.peer_manager.disconnect_all_peers();
        let _ = self.shutdown_tx.send(true);
        if let Some(handle) = self.listener.lock().expect("listener mutex poisoned").take() {
            let _ = handle.await;
        }
        if let Some(handle) = self.chain_sync.lock().expect("chain sync mutex poisoned").take() {
            let _ = handle.await;
        }
        if let Some(background) = self.background.lock().expect("background mutex poisoned").take()
        {
            if let Err(err) = background.stop() {
                eprintln!("[{}] channel manager persist failed: {err}", self.name);
            }
        }
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.shutdown_tx.send(true);
        let Ok(mut background) = self.background.lock() else {
            return;
        };
        if let Some(background) = background.take() {
            if let Err(err) = background.stop() {
                eprintln!("[{}] channel manager persist failed: {err}", self.name);
            }
        }
    }
}

pub async fn build_node(
    name: &'static str, data_dir: PathBuf, listen_port: u16, bitcoind: Arc<BitcoindFacade>,
) -> Arc<Node> {
    let logger = Arc::new(StdLogger { name: name.to_string(), level: Level::Info });
    let _ = std::fs::create_dir_all(&data_dir);
    let seed = load_or_create_seed(&data_dir);

    let now = SystemTime::now().duration_since(UNIX_EPOCH).expect("system clock to be valid");
    let keys_manager =
        Arc::new(KeysManager::new(&seed, now.as_secs(), now.subsec_nanos(), true));

    let fs_store = Arc::new(FilesystemStore::new(data_dir.clone()));
    let manager_bytes = match fs_store.read(
        CHANNEL_MANAGER_PERSISTENCE_PRIMARY_NAMESPACE,
        CHANNEL_MANAGER_PERSISTENCE_SECONDARY_NAMESPACE,
        CHANNEL_MANAGER_PERSISTENCE_KEY,
    ) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == lightning::io::ErrorKind::NotFound => None,
        Err(err) => panic!("failed to read channel manager: {}", err),
    };

    let persister = MonitorUpdatingPersisterAsync::new(
        KVStoreSyncWrapper(Arc::clone(&fs_store)),
        RuntimeSpawner(tokio::runtime::Handle::current()),
        Arc::clone(&logger),
        1000,
        Arc::clone(&keys_manager),
        Arc::clone(&keys_manager),
        Arc::clone(&bitcoind),
        Arc::clone(&bitcoind),
    );
    let monitors = if manager_bytes.is_some() {
        persister
            .read_all_channel_monitors_with_updates()
            .await
            .expect("failed to read channel monitors")
    } else {
        Vec::new()
    };

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
    let user_config = node_user_config();

    let channel_manager = if let Some(bytes) = manager_bytes {
        let monitor_refs: Vec<_> = monitors.iter().map(|(_, monitor)| monitor).collect();
        let read_args = ChannelManagerReadArgs::new(
            Arc::clone(&keys_manager),
            Arc::clone(&keys_manager),
            Arc::clone(&keys_manager),
            Arc::clone(&bitcoind),
            Arc::clone(&chain_monitor),
            Arc::clone(&bitcoind),
            Arc::clone(&router),
            Arc::clone(&message_router),
            Arc::clone(&logger),
            user_config,
            monitor_refs,
        );
        let mut cursor = lightning::io::Cursor::new(bytes);
        let (_block_hash, manager) =
            <(bitcoin::BlockHash, ChannelManagerT)>::read(&mut cursor, read_args)
                .expect("failed to deserialize channel manager");
        for (_, monitor) in monitors {
            let channel_id = monitor.channel_id();
            chain_monitor
                .watch_channel(channel_id, monitor)
                .expect("failed to watch restored channel monitor");
        }
        Arc::new(manager)
    } else {
        let chain_params = ChainParameters {
            network: bitcoin::Network::Regtest,
            best_block: chain_tip.to_best_block(),
        };
        Arc::new(ChannelManagerT::new(
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
        ))
    };

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

    let (events_tx, _) = event_channel();
    let events = Arc::new(Mutex::new(Vec::new()));
    let handler = NodeEventHandler {
        name: name.to_string(),
        channel_manager: Arc::clone(&channel_manager),
        bitcoind: Arc::clone(&bitcoind),
        events: Arc::clone(&events),
        broadcast: events_tx.clone(),
    };
    let onion: Option<
        Arc<
            OnionMessenger<
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<StdLogger>,
                Arc<ChannelManagerT>,
                Arc<
                    DefaultMessageRouter<
                        Arc<NetworkGraph<Arc<StdLogger>>>,
                        Arc<StdLogger>,
                        Arc<KeysManager>,
                    >,
                >,
                Arc<IgnoringMessageHandler>,
                Arc<IgnoringMessageHandler>,
                Arc<IgnoringMessageHandler>,
                Arc<IgnoringMessageHandler>,
            >,
        >,
    > = None;
    let gossip = GossipSync::<
        &P2PGossipSync<
            &'static NetworkGraph<Arc<StdLogger>>,
            &'static (dyn UtxoLookup + Send + Sync),
            Arc<StdLogger>,
        >,
        &RapidGossipSync<&'static NetworkGraph<Arc<StdLogger>>, Arc<StdLogger>>,
        &'static NetworkGraph<Arc<StdLogger>>,
        &'static (dyn UtxoLookup + Send + Sync),
        Arc<StdLogger>,
    >::none();
    let liquidity: Option<
        Arc<
            LiquidityManagerSync<
                Arc<KeysManager>,
                Arc<KeysManager>,
                Arc<ChannelManagerT>,
                Arc<dyn Filter + Send + Sync>,
                Arc<RetryStore>,
                Arc<DefaultTimeProvider>,
                Arc<BitcoindFacade>,
            >,
        >,
    > = None;
    let sweeper: Option<
        Arc<
            OutputSweeperSync<
                Arc<BitcoindFacade>,
                Arc<NoChange>,
                Arc<BitcoindFacade>,
                Arc<dyn Filter + Send + Sync>,
                Arc<RetryStore>,
                Arc<StdLogger>,
                Arc<KeysManager>,
            >,
        >,
    > = None;
    let scorer_slot: Option<
        Arc<
            ScorerSlot<
                ProbabilisticScorer<Arc<NetworkGraph<Arc<StdLogger>>>, Arc<StdLogger>>,
            >,
        >,
    > = None;
    let background = BackgroundProcessor::start(
        Arc::new(RetryStore(Arc::clone(&fs_store))),
        handler,
        Arc::clone(&chain_monitor),
        Arc::clone(&channel_manager),
        onion,
        gossip,
        Arc::clone(&peer_manager),
        liquidity,
        sweeper,
        Arc::clone(&logger),
        scorer_slot,
    );

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let listen_addr = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, listen_port));
    let listener = start_listening(Arc::clone(&peer_manager), listen_port, shutdown_rx.clone());
    let chain_sync = start_chain_sync(
        Arc::clone(&bitcoind),
        chain_tip,
        Arc::clone(&chain_monitor),
        Arc::clone(&channel_manager),
        bitcoin::Network::Regtest,
        shutdown_rx,
    );
    reconnect_saved_peers(&data_dir, Arc::clone(&peer_manager));

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
        events,
        bitcoind,
        events_tx,
        data_dir,
        shutdown_tx,
        listener: Mutex::new(Some(listener)),
        chain_sync: Mutex::new(Some(chain_sync)),
        background: Mutex::new(Some(background)),
    });
    println!("SPIKE_INFO: built node {}", name);
    node
}

struct NoChange;

impl ChangeDestinationSourceSync for NoChange {
    fn get_change_destination_script(&self) -> Result<bitcoin::blockdata::script::ScriptBuf, ()> {
        Err(())
    }
}

pub struct RuntimeSpawner(tokio::runtime::Handle);

impl FutureSpawner for RuntimeSpawner {
    fn spawn<T: std::future::Future<Output = ()> + Send + 'static>(&self, future: T) {
        // BackgroundProcessor calls this from its own thread, where tokio::spawn has no reactor.
        self.0.spawn(future);
    }
}

struct RetryStore(Arc<FilesystemStore>);

impl KVStoreSync for RetryStore {
    fn read(
        &self, primary_namespace: &str, secondary_namespace: &str, key: &str,
    ) -> Result<Vec<u8>, lightning::io::Error> {
        self.0.read(primary_namespace, secondary_namespace, key)
    }

    fn write(
        &self, primary_namespace: &str, secondary_namespace: &str, key: &str, buf: Vec<u8>,
    ) -> Result<(), lightning::io::Error> {
        let mut last_err = None;
        for _ in 0..8 {
            match self.0.write(primary_namespace, secondary_namespace, key, buf.clone()) {
                Ok(()) => return Ok(()),
                Err(err) => {
                    last_err = Some(err);
                    std::thread::sleep(Duration::from_millis(50));
                },
            }
        }
        Err(last_err.expect("retry loop records the last write error"))
    }

    fn remove(
        &self, primary_namespace: &str, secondary_namespace: &str, key: &str, lazy: bool,
    ) -> Result<(), lightning::io::Error> {
        self.0.remove(primary_namespace, secondary_namespace, key, lazy)
    }

    fn list(
        &self, primary_namespace: &str, secondary_namespace: &str,
    ) -> Result<Vec<String>, lightning::io::Error> {
        self.0.list(primary_namespace, secondary_namespace)
    }
}

struct ScorerSlot<T>(std::sync::RwLock<T>);

impl<T: Score + Writeable> Writeable for ScorerSlot<T> {
    fn write<W: Writer>(&self, writer: &mut W) -> Result<(), lightning::io::Error> {
        self.0.read().expect("scorer lock poisoned").write(writer)
    }
}

impl<'a, T: Score + 'a> lightning::routing::scoring::LockableScore<'a> for ScorerSlot<T> {
    type ScoreUpdate = T;
    type ScoreLookUp = T;
    type WriteLocked = std::sync::RwLockWriteGuard<'a, T>;
    type ReadLocked = std::sync::RwLockReadGuard<'a, T>;

    fn read_lock(&'a self) -> Self::ReadLocked {
        self.0.read().expect("scorer lock poisoned")
    }

    fn write_lock(&'a self) -> Self::WriteLocked {
        self.0.write().expect("scorer lock poisoned")
    }
}

fn node_user_config() -> UserConfig {
    UserConfig {
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
    }
}

fn load_or_create_seed(data_dir: &Path) -> [u8; 32] {
    let seed_path = data_dir.join("keys_seed");
    match std::fs::read(&seed_path) {
        Ok(existing_seed) if existing_seed.len() == 32 => {
            let mut seed_bytes = [0u8; 32];
            seed_bytes.copy_from_slice(&existing_seed);
            seed_bytes
        },
        Ok(existing) => {
            panic!("seed file corrupted (len={}), refusing to overwrite", existing.len());
        },
        Err(ref err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut seed_bytes = [0u8; 32];
            rand::thread_rng().fill_bytes(&mut seed_bytes);
            let _ = std::fs::create_dir_all(data_dir);
            let _ = std::fs::write(&seed_path, &seed_bytes);
            seed_bytes
        },
        Err(err) => panic!("failed to read seed: {}", err),
    }
}

fn peers_path(data_dir: &Path) -> PathBuf {
    data_dir.join("peers.json")
}

fn read_peers(data_dir: &Path) -> Vec<PeerRecord> {
    let path = peers_path(data_dir);
    let Ok(bytes) = std::fs::read(&path) else {
        return Vec::new();
    };
    serde_json::from_slice(&bytes).unwrap_or_default()
}

fn write_peers(data_dir: &Path, peers: &[PeerRecord]) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(peers).map_err(|err| err.to_string())?;
    let path = peers_path(data_dir);
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &bytes).map_err(|err| err.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|err| err.to_string())
}

fn reconnect_saved_peers(data_dir: &Path, peer_manager: Arc<PeerManagerT>) {
    for peer in read_peers(data_dir) {
        let Ok(pubkey) = PublicKey::from_str(&peer.pubkey) else {
            continue;
        };
        let Ok(addr) = peer.addr.parse() else {
            continue;
        };
        let peer_manager = Arc::clone(&peer_manager);
        tokio::spawn(async move {
            let _ = connect_peer(peer_manager, pubkey, addr).await;
        });
    }
}

async fn connect_peer(
    peer_manager: Arc<PeerManagerT>, pubkey: PublicKey, addr: SocketAddr,
) -> Result<(), String> {
    match lightning_net_tokio::connect_outbound(peer_manager, pubkey, addr).await {
        Some(_) => Ok(()),
        None => Err("failed to connect to peer".to_string()),
    }
}

fn start_listening(
    peer_manager: Arc<PeerManagerT>, listen_port: u16, mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{}", listen_port))
            .await
            .expect("failed to bind LDK listen port");
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                accepted = listener.accept() => {
                    let tcp_stream = match accepted {
                        Ok(stream) => stream.0,
                        Err(err) => {
                            eprintln!("accept failed: {err}");
                            continue;
                        },
                    };
                    let peer_manager = Arc::clone(&peer_manager);
                    tokio::spawn(async move {
                        lightning_net_tokio::setup_inbound(
                            peer_manager,
                            tcp_stream.into_std().unwrap(),
                        )
                        .await;
                    });
                }
            }
        }
    })
}

fn start_chain_sync(
    bitcoind: Arc<BitcoindFacade>, chain_tip: ValidatedBlockHeader,
    chain_monitor: Arc<ChainMonitorT>, channel_manager: Arc<ChannelManagerT>,
    network: bitcoin::Network, mut shutdown: watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let chain_poller = ChainPoller::new(bitcoind.as_ref(), network);
        let mut cache = UnboundedCache::new();
        let chain_listener = (chain_monitor, channel_manager);
        let mut spv_client = SpvClient::new(chain_tip, chain_poller, &mut cache, &chain_listener);
        loop {
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                polled = spv_client.poll_best_tip() => {
                    if let Err(err) = polled {
                        eprintln!("chain poll failed: {err:?}; will retry");
                        continue;
                    }
                }
            }
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(500)) => {}
            }
        }
    })
}
