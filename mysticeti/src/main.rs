//! Mysticeti node binary: CLI, Iroh endpoint, protocol handler, core loop.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::Parser;
use iroh::protocol::Router;
use mysticeti_core::{
    Authority, AuthorityIndex, BaseStatement, Committee, Core, EpochNumber, Identifier,
    Parameters, PublicKey, SimpleCommitObserver, Transaction,
};
use mysticeti_net::{MysticetiProtocol, PeerIdentity, MYSTICETI_ALPN};
use serde::Deserialize;
use tokio::sync::mpsc;
use tracing::info;

#[derive(Parser)]
#[command(name = "mysticeti")]
#[command(about = "Mysticeti consensus node with Iroh transport")]
struct Cli {
    /// Authority index of this node (0..n-1).
    #[arg(long, default_value = "0")]
    authority_index: u64,

    /// Path to config file (optional; uses built-in test committee if not set).
    #[arg(long)]
    config: Option<PathBuf>,

    /// Storage directory for WAL and state.
    #[arg(long, default_value = "./data")]
    storage_dir: PathBuf,

    /// Optional address to listen on for transaction submissions (e.g. 0.0.0.0:7000).
    #[arg(long)]
    submit_listen_addr: Option<String>,

    /// Core loop interval in milliseconds (try_new_block/try_commit cadence). Default: 100.
    #[arg(long, default_value = "100")]
    loop_interval_ms: u64,
}

fn make_test_committee() -> (Committee, Parameters) {
    let mut authorities = Vec::new();
    let mut identifiers = Vec::new();
    for i in 0u8..4 {
        let pk = PublicKey([i; 32]);
        authorities.push(Authority {
            stake: 1,
            public_key: pk.clone(),
            hostname: format!("validator-{}", i),
        });
        identifiers.push(Identifier {
            public_key: pk,
            network_address: format!("127.0.0.1:{}", 9000 + i as u32),
            metrics_address: format!("127.0.0.1:{}", 9100 + i as u32),
        });
    }
    let committee = Committee {
        authorities,
        epoch: EpochNumber(0),
        validity_threshold: 2,
        quorum_threshold: 3,
    };
    let params = Parameters {
        identifiers,
        ..Default::default()
    };
    (committee, params)
}

/// Simple handler that turns enqueued transactions into `Share(Transaction)` statements.
/// Internally it uses an `Arc<Mutex<_>>` so multiple tasks can enqueue transactions.
#[derive(Clone, Default)]
struct TxHandler {
    queue: Arc<std::sync::Mutex<Vec<Transaction>>>,
}

impl TxHandler {
    fn new() -> Self {
        Self {
            queue: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    fn submit_transaction(&self, payload: Vec<u8>) {
        let mut guard = self.queue.lock().unwrap();
        guard.push(Transaction(payload));
    }
}

impl mysticeti_core::BlockHandler for TxHandler {
    fn statements_for_block(
        &self,
        _authority: mysticeti_core::AuthorityIndex,
        _round: mysticeti_core::RoundNumber,
        _includes: &[mysticeti_core::BlockReference],
    ) -> Vec<BaseStatement> {
        let mut guard = self.queue.lock().unwrap();
        let mut stmts = Vec::new();
        for tx in guard.drain(..) {
            stmts.push(BaseStatement::Share(tx));
        }
        stmts
    }
}

#[derive(Debug, Deserialize)]
struct CommitteeMemberConfig {
    name: String,
    stake: u64,
    network_address: String,
    metrics_address: String,
}

#[derive(Debug, Deserialize)]
struct CommitteeConfig {
    epoch: u64,
    validity_threshold: u64,
    quorum_threshold: u64,
    members: Vec<CommitteeMemberConfig>,
}

#[derive(Debug, Deserialize)]
struct ParametersConfig {
    wave_length: Option<u64>,
    leader_timeout_ms: Option<u64>,
    rounds_in_epoch: Option<u64>,
    shutdown_grace_period_ms: Option<u64>,
    number_of_leaders: Option<u32>,
    enable_pipelining: Option<bool>,
    store_retain_rounds: Option<u64>,
    enable_cleanup: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct NodeConfig {
    committee: CommitteeConfig,
    parameters: Option<ParametersConfig>,
}

fn load_config(path: &Path) -> anyhow::Result<(Committee, Parameters)> {
    let data = fs::read_to_string(path)?;
    let cfg: NodeConfig = toml::from_str(&data)?;

    let mut authorities = Vec::new();
    let mut identifiers = Vec::new();
    for (i, m) in cfg.committee.members.iter().enumerate() {
        let pk = PublicKey([i as u8; 32]);
        authorities.push(Authority {
            stake: m.stake,
            public_key: pk.clone(),
            hostname: m.name.clone(),
        });
        identifiers.push(Identifier {
            public_key: pk,
            network_address: m.network_address.clone(),
            metrics_address: m.metrics_address.clone(),
        });
    }
    let committee = Committee {
        authorities,
        epoch: EpochNumber(cfg.committee.epoch),
        validity_threshold: cfg.committee.validity_threshold,
        quorum_threshold: cfg.committee.quorum_threshold,
    };
    let mut params = Parameters {
        identifiers,
        ..Default::default()
    };
    if let Some(p) = cfg.parameters {
        if let Some(v) = p.wave_length {
            params.wave_length = v;
        }
        if let Some(v) = p.leader_timeout_ms {
            params.leader_timeout_ms = v;
        }
        if let Some(v) = p.rounds_in_epoch {
            params.rounds_in_epoch = v;
        }
        if let Some(v) = p.shutdown_grace_period_ms {
            params.shutdown_grace_period_ms = v;
        }
        if let Some(v) = p.number_of_leaders {
            params.number_of_leaders = v;
        }
        if let Some(v) = p.enable_pipelining {
            params.enable_pipelining = v;
        }
        if let Some(v) = p.store_retain_rounds {
            params.store_retain_rounds = v;
        }
        if let Some(v) = p.enable_cleanup {
            params.enable_cleanup = v;
        }
    }
    Ok((committee, params))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let (committee, params) = match &cli.config {
        Some(path) => load_config(path)?,
        None => make_test_committee(),
    };

    let authority_index = AuthorityIndex(cli.authority_index);
    let handler = TxHandler::new();
    let submit_handler = handler.clone();
    let core: Arc<Core<TxHandler>> = Arc::new(Core::new(
        params.clone(),
        committee.clone(),
        authority_index,
        None,
        handler,
    ));

    let (inbox_tx, mut inbox_rx) = mpsc::unbounded_channel();
    let peer_identity = PeerIdentity::new(committee.clone());
    let protocol = MysticetiProtocol::new(peer_identity.clone(), inbox_tx);

    let endpoint = iroh::Endpoint::builder()
        .alpns(vec![MYSTICETI_ALPN.to_vec()])
        .bind()
        .await?;
    endpoint.online().await;

    let router = Router::builder(endpoint.clone())
        .accept(MYSTICETI_ALPN, protocol.clone())
        .spawn();

    let core_inbox = core.clone();
    tokio::spawn(async move {
        while let Some((msg, from)) = inbox_rx.recv().await {
            match msg {
                mysticeti_core::NetworkMessage::Blocks(blocks) => {
                    let _ = core_inbox.add_blocks(blocks);
                }
                mysticeti_core::NetworkMessage::RequestBlocks(refs) => {
                    let response = mysticeti_net::Syncer::new()
                        .handle_request_blocks(core_inbox.manager().store(), &refs);
                    protocol.send(from, response);
                }
                _ => {}
            }
        }
    });

    let (commit_tx, mut commit_rx) = tokio::sync::mpsc::unbounded_channel();
    let observer = std::sync::Arc::new(SimpleCommitObserver::new(commit_tx));

    let loop_interval = cli.loop_interval_ms;
    let core_loop = core.clone();
    let observer_loop = observer.clone();
    tokio::spawn(async move {
        loop {
            let _ = core_loop.try_new_block();
            core_loop.try_commit(observer_loop.as_ref());
            tokio::time::sleep(tokio::time::Duration::from_millis(loop_interval)).await;
        }
    });

    // Light logging of committed blocks/tx for demo/observability.
    tokio::spawn(async move {
        use mysticeti_core::BaseStatement;
        let mut total_blocks: u64 = 0;
        let mut total_txs: u64 = 0;
        while let Some(subdag) = commit_rx.recv().await {
            let batch_blocks = subdag.blocks.len() as u64;
            let mut batch_txs = 0u64;
            for b in &subdag.blocks {
                batch_txs += b
                    .statements
                    .iter()
                    .filter(|s| matches!(s, BaseStatement::Share(_)))
                    .count() as u64;
            }
            total_blocks += batch_blocks;
            total_txs += batch_txs;
            info!(
                "committed subdag: {} blocks, {} tx (totals: {} blocks, {} tx)",
                batch_blocks, batch_txs, total_blocks, total_txs
            );
        }
    });

    // Optional TCP listener for transaction submissions (used by Docker client).
    if let Some(addr) = &cli.submit_listen_addr {
        let addr = addr.clone();
        let handler = submit_handler.clone();
        tokio::spawn(async move {
            if let Ok(listener) = tokio::net::TcpListener::bind(&addr).await {
                info!("listening for submissions on {}", addr);
                loop {
                    match listener.accept().await {
                        Ok((mut socket, _peer)) => {
                            let handler = handler.clone();
                            tokio::spawn(async move {
                                use tokio::io::{AsyncBufReadExt, BufReader};
                                let reader = BufReader::new(&mut socket);
                                let mut lines = reader.lines();
                                while let Ok(Some(line)) = lines.next_line().await {
                                    handler.submit_transaction(line.into_bytes());
                                }
                            });
                        }
                        Err(e) => {
                            tracing::warn!("accept error on {}: {}", addr, e);
                        }
                    }
                }
            } else {
                tracing::warn!("failed to bind submission listener on {}", addr);
            }
        });
    }

    info!(
        "mysticeti node running (authority_index={}, submit_listen_addr={:?})",
        cli.authority_index, cli.submit_listen_addr
    );
    tokio::signal::ctrl_c().await?;
    router.shutdown().await?;
    Ok(())
}
