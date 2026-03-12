//! Example: in-process 4-node Mysticeti network and simple transaction submission.
//!
//! This example spins up four `Core` instances in the same process, wires them together
//! with an in-memory broadcast, and shows how to submit transactions and observe commits.
//!
//! Run with:
//! `cargo run -p mysticeti --example four_node_inproc`

use std::sync::{Arc, Mutex};
use std::time::Duration;

use mysticeti_core::{
    Authority, AuthorityIndex, BaseStatement, Committee, Core, EpochNumber, Identifier, Parameters,
    SimpleCommitObserver, Transaction,
};
use tokio::sync::mpsc;

/// Simple handler that turns enqueued transactions into `Share(Transaction)` statements.
/// Internally it uses an `Arc<Mutex<...>>` so that cloned handlers share the same queue.
#[derive(Clone, Default)]
struct TxHandler {
    queue: Arc<Mutex<Vec<Transaction>>>,
}

impl TxHandler {
    fn new() -> Self {
        Self {
            queue: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Enqueue a transaction to be included in the next blocks for this authority.
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
        // Drain all queued transactions into this block as Share statements.
        let mut guard = self.queue.lock().unwrap();
        let mut stmts = Vec::new();
        for tx in guard.drain(..) {
            stmts.push(BaseStatement::Share(tx));
        }
        stmts
    }
}

/// Build a simple 4-authority committee and matching parameters.
fn make_test_committee() -> (Committee, Parameters) {
    let mut authorities = Vec::new();
    let mut identifiers = Vec::new();
    for i in 0u8..4 {
        let pk = mysticeti_core::PublicKey([i; 32]);
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
        // Use a shorter wave so that decision-round blocks (round 1) include
        // the leader-round block (round 0) using the default `required_includes_for_round`.
        wave_length: 2,
        ..Default::default()
    };
    (committee, params)
}

/// Submit a few demo transactions into different authorities.
fn submit_demo_transactions(handlers: &[Arc<TxHandler>]) {
    handlers[0].submit_transaction(b"tx-0-from-client-A".to_vec());
    handlers[1].submit_transaction(b"tx-1-from-client-B".to_vec());
    handlers[2].submit_transaction(b"tx-2-from-client-C".to_vec());
    handlers[3].submit_transaction(b"tx-3-from-client-D".to_vec());
    handlers[0].submit_transaction(b"tx-4-from-client-A".to_vec());
}

#[tokio::main]
async fn main() {
    // Build committee and params, then four cores with their own TxHandler.
    let (committee, params) = make_test_committee();
    let mut handlers: Vec<Arc<TxHandler>> = Vec::new();
    let mut cores: Vec<Arc<Core<TxHandler>>> = Vec::new();
    for i in 0..4u64 {
        let handler = Arc::new(TxHandler::new());
        // Clone the inner TxHandler so the core owns its handler by value.
        let core_handler: TxHandler = (*handler).clone();
        let core = Arc::new(Core::new(
            params.clone(),
            committee.clone(),
            AuthorityIndex(i),
            None,
            core_handler,
        ));
        handlers.push(handler);
        cores.push(core);
    }

    // Channel to receive committed subdags.
    let (commit_tx, mut commit_rx) = mpsc::unbounded_channel();
    let observer = Arc::new(SimpleCommitObserver::new(commit_tx));

    // Spawn a background task that repeatedly:
    // - asks each authority to propose a new block
    // - broadcasts all newly created blocks to every core
    // - runs consensus and emits commits via the observer
    let cores_for_loop = cores.clone();
    let observer_for_loop = observer.clone();
    tokio::spawn(async move {
        loop {
            // Each authority tries to create a new block (using its TxHandler).
            let mut new_blocks = Vec::new();
            for core in &cores_for_loop {
                if let Some(block) = core.try_new_block() {
                    new_blocks.push(block);
                }
            }
            // Broadcast all newly created blocks to every core (in-process "network").
            if !new_blocks.is_empty() {
                for core in &cores_for_loop {
                    core.add_blocks(new_blocks.clone());
                }
            }
            // Run consensus to try to commit leaders.
            for core in &cores_for_loop {
                core.try_commit(observer_for_loop.as_ref());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });

    // Simulate client traffic by submitting a few transactions to various authorities.
    submit_demo_transactions(&handlers);

    println!("Waiting for commits (Ctrl-C to exit)...");
    // Print committed blocks and the transactions they contain.
    while let Some(subdag) = commit_rx.recv().await {
        println!("New committed subdag with {} blocks:", subdag.blocks.len());
        for block in &subdag.blocks {
            println!(
                "  Authority {} round {} with {} statements",
                block.creator.0,
                block.round,
                block.statements.len()
            );
            for stmt in &block.statements {
                if let BaseStatement::Share(tx) = stmt {
                    if let Ok(s) = std::str::from_utf8(&tx.0) {
                        println!("    Share(tx = \"{}\")", s);
                    } else {
                        println!("    Share(tx = {:?})", tx.0);
                    }
                }
            }
        }
    }
}
