use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use bitcoin::blockdata::transaction::Transaction;
use bitcoin::consensus::encode;
use bitcoin::hash_types::BlockHash;
use lightning::chain::chaininterface::{BroadcasterInterface, ConfirmationTarget, FeeEstimator};
use lightning_block_sync::http::HttpEndpoint;
use lightning_block_sync::rpc::RpcClient;
use lightning_block_sync::{AsyncBlockSourceResult, BlockData, BlockHeaderData, BlockSource};
use serde_json::{json, Value};
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Handle;

pub struct BitcoindFacade {
    pub rpc: Arc<RpcClient>,
    runtime: Handle,
}

impl BitcoindFacade {
    pub fn new(host: String, port: u16, rpc_user: String, rpc_password: String) -> BitcoindFacade {
        let http_endpoint = HttpEndpoint::for_host(host).with_port(port);
        let credentials = B64.encode(format!("{}:{}", rpc_user, rpc_password));
        let rpc = Arc::new(RpcClient::new(&credentials, http_endpoint));
        BitcoindFacade { rpc, runtime: Handle::current() }
    }

    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime.block_on(future)
    }

    pub async fn wait_for_bitcoind(&self, timeout_secs: u64) -> Result<(), String> {
        let deadline = Instant::now() + Duration::from_secs(timeout_secs);
        loop {
            if Instant::now() >= deadline {
                return Err("bitcoind did not answer the first getblockchaininfo call".to_string());
            }
            match self.rpc.call_method::<Value>("getblockchaininfo", &[]).await {
                Ok(info) => {
                    println!(
                        "SPIKE_INFO: bitcoind online, chain={}",
                        info["chain"].as_str().unwrap_or("unknown")
                    );
                    return Ok(());
                },
                Err(_) => tokio::time::sleep(Duration::from_millis(500)).await,
            }
        }
    }

    pub async fn ensure_wallet(&self, wallet_name: &str) -> Result<(), String> {
        match self.rpc.call_method::<Value>("createwallet", &[json!(wallet_name)]).await {
            Ok(_) => {
                println!("SPIKE_INFO: wallet created ({})", wallet_name);
                Ok(())
            },
            Err(e) if format!("{:?}", e).contains("already exists") => Ok(()),
            Err(e) => Err(format!("{:?}", e)),
        }
    }

    pub async fn get_block_height(&self) -> Result<u64, String> {
        let info: Value =
            self.rpc.call_method("getblockchaininfo", &[]).await.map_err(|e| format!("{:?}", e))?;
        match info["blocks"].as_u64() {
            Some(blocks) => Ok(blocks),
            None => Err("Missing 'blocks' field in getblockchaininfo response".to_string()),
        }
    }

    pub async fn generate_blocks(&self, count: u64) -> Result<(), String> {
        let addr: Value = self
            .rpc
            .call_method("getnewaddress", &[json!("spike-miner")])
            .await
            .map_err(|e| format!("{:?}", e))?;
        let addr_str = addr.as_str().ok_or("getnewaddress returned no address")?;
        self.rpc
            .call_method::<Value>("generatetoaddress", &[json!(count), json!(addr_str)])
            .await
            .map_err(|e| format!("{:?}", e))?;
        Ok(())
    }

    pub async fn create_funding_transaction(
        &self, output_script: &bitcoin::blockdata::script::ScriptBuf, channel_value_satoshis: u64,
    ) -> Result<Transaction, String> {
        let address = bitcoin::address::Address::from_script(
            output_script,
            bitcoin::network::Network::Regtest,
        )
        .map_err(|e| format!("{:?}", e))?;
        let address_string = address.to_string();
        let outputs = json!([{ address_string: channel_value_satoshis as f64 / 100_000_000.0 }]);
        let raw_tx: Value = self
            .rpc
            .call_method("createrawtransaction", &[json!([]), outputs])
            .await
            .map_err(|e| format!("{:?}", e))?;
        let raw_hex = raw_tx.as_str().ok_or("createrawtransaction returned no hex")?;
        let options = json!({
            "fee_rate": 2000.0_f64 / 250.0,
            "replaceable": false,
        });
        let funded: Value = self
            .rpc
            .call_method("fundrawtransaction", &[json!(raw_hex), options])
            .await
            .map_err(|e| format!("{:?}", e))?;
        let funded_hex = funded["hex"].as_str().ok_or("fundrawtransaction returned no hex")?;
        let signed: Value = self
            .rpc
            .call_method("signrawtransactionwithwallet", &[json!(funded_hex)])
            .await
            .map_err(|e| format!("{:?}", e))?;
        if signed["complete"].as_bool() != Some(true) {
            return Err("signrawtransactionwithwallet did not complete the funding transaction".to_string());
        }
        let signed_hex = signed["hex"].as_str().ok_or("signrawtransaction returned no hex")?;
        let tx_bytes = hex::decode(signed_hex).map_err(|e| format!("{:?}", e))?;
        let tx: Transaction = encode::deserialize::<Transaction>(&tx_bytes)
            .map_err(|e| format!("{:?}", e))?;
        Ok(tx)
    }
}

impl BlockSource for BitcoindFacade {
    fn get_header<'a>(
        &'a self, header_hash: &'a BlockHash, height_hint: Option<u32>,
    ) -> AsyncBlockSourceResult<'a, BlockHeaderData> {
        Box::pin(async move { self.rpc.get_header(header_hash, height_hint).await })
    }

    fn get_block<'a>(
        &'a self, header_hash: &'a BlockHash,
    ) -> AsyncBlockSourceResult<'a, BlockData> {
        Box::pin(async move { self.rpc.get_block(header_hash).await })
    }

    fn get_best_block<'a>(&'a self) -> AsyncBlockSourceResult<'a, (BlockHash, Option<u32>)> {
        Box::pin(async move { self.rpc.get_best_block().await })
    }
}

impl FeeEstimator for BitcoindFacade {
    fn get_est_sat_per_1000_weight(&self, confirmation_target: ConfirmationTarget) -> u32 {
        match confirmation_target {
            ConfirmationTarget::MaximumFeeEstimate => 50000,
            ConfirmationTarget::UrgentOnChainSweep => 25000,
            ConfirmationTarget::MinAllowedAnchorChannelRemoteFee => 253,
            ConfirmationTarget::MinAllowedNonAnchorChannelRemoteFee => 253,
            ConfirmationTarget::AnchorChannelFee => 2000,
            ConfirmationTarget::NonAnchorChannelFee => 2000,
            ConfirmationTarget::ChannelCloseMinimum => 1000,
            ConfirmationTarget::OutputSpendingFee => 1000,
        }
    }
}

impl BroadcasterInterface for BitcoindFacade {
    fn broadcast_transactions(&self, txs: &[&Transaction]) {
        let txn = txs.iter().map(|tx| encode::serialize_hex(tx)).collect::<Vec<_>>();
        let rpc_client = Arc::clone(&self.rpc);
        self.runtime.spawn(async move {
            let res = if txn.len() == 1 {
                let tx_json = json!(txn[0]);
                rpc_client.call_method::<Value>("sendrawtransaction", &[tx_json]).await
            } else {
                let tx_json = json!(txn);
                rpc_client.call_method::<Value>("submitpackage", &[tx_json]).await
            };
            if let Err(e) = res {
                println!("SPIKE_WARN: failed to broadcast a transaction (usually harmless): {:?}", e);
            }
        });
    }
}
