//! Mysticeti node binary: CLI, Iroh endpoint, protocol handler, core loop.

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use iroh::protocol::Router;
use mysticeti_core::{
    AuthorityIndex, Committee, Core, NoOpBlockHandler, Parameters, SimpleCommitObserver,
};
use mysticeti_net::{MysticetiProtocol, PeerIdentity, MYSTICETI_ALPN};
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
}

fn make_test_committee() -> (Committee, Parameters) {
    use mysticeti_core::PublicKey;
    use mysticeti_core::{Authority, EpochNumber, Identifier};
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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let (committee, params) = match &cli.config {
        Some(_path) => {
            // TODO: load from file
            make_test_committee()
        }
        None => make_test_committee(),
    };

    let authority_index = AuthorityIndex(cli.authority_index);
    let handler = NoOpBlockHandler;
    let core: Arc<Core<NoOpBlockHandler>> = Arc::new(Core::new(
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

    let (commit_tx, _commit_rx) = tokio::sync::mpsc::unbounded_channel();
    let observer = std::sync::Arc::new(SimpleCommitObserver::new(commit_tx));

    let core_loop = core.clone();
    let observer_loop = observer.clone();
    tokio::spawn(async move {
        loop {
            let _ = core_loop.try_new_block();
            core_loop.try_commit(observer_loop.as_ref());
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    });

    info!(
        "mysticeti node running (authority_index={})",
        cli.authority_index
    );
    tokio::signal::ctrl_c().await?;
    router.shutdown().await?;
    Ok(())
}
