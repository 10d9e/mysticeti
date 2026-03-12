//! TPS benchmark: run multi-validator scenario for a fixed duration, report committed blocks/sec and transactions/sec.

use criterion::{criterion_group, criterion_main, Criterion};
use mysticeti_core::{
    Authority, AuthorityIndex, BaseStatement, BlockDigest, BlockReference, BlockSignature,
    Committee, Core, Data, EpochNumber, NoOpBlockHandler, Parameters, SimpleCommitObserver,
    StatementBlock, Transaction,
};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

fn make_committee(n: usize) -> Committee {
    let mut authorities = Vec::new();
    for i in 0..n {
        authorities.push(Authority {
            stake: 1,
            public_key: mysticeti_core::PublicKey([i as u8; 32]),
            hostname: format!("v{}", i),
        });
    }
    let n = n as u64;
    Committee {
        authorities,
        epoch: EpochNumber(0),
        validity_threshold: n / 2 + 1,
        quorum_threshold: 2 * n / 3 + 1,
    }
}

fn make_params(number_of_leaders: u32) -> Parameters {
    Parameters {
        wave_length: 3,
        number_of_leaders,
        ..Default::default()
    }
}

/// Build a block with `transactions_per_block` Share(Transaction) statements.
fn make_block_with_txs(
    creator: AuthorityIndex,
    round: u64,
    includes: Vec<BlockReference>,
    transactions_per_block: usize,
) -> Data<StatementBlock> {
    let statements: Vec<BaseStatement> = (0..transactions_per_block)
        .map(|_| BaseStatement::Share(Transaction(vec![0u8; 32])))
        .collect();
    Data::new(StatementBlock::new(
        creator,
        round,
        BlockDigest([0u8; 32]),
        includes,
        statements,
        EpochNumber(0),
        BlockSignature(vec![]),
    ))
}

/// Run one round: each authority creates a block (with txs), broadcast to all cores.
/// Refs are taken from the store (last 2 rounds only) to avoid O(rounds²) cloning.
fn run_round(
    cores: &[Core<NoOpBlockHandler>],
    transactions_per_block: usize,
) -> Vec<Data<StatementBlock>> {
    let n = cores.len();
    let store = cores[0].manager().store();
    let round = (0..n)
        .filter_map(|i| cores[i].manager().highest_round(AuthorityIndex(i as u64)))
        .max()
        .map(|r| r + 1)
        .unwrap_or(0);
    let prev_refs: Vec<BlockReference> = if round == 0 {
        vec![]
    } else if round == 1 {
        store.references_at_round(0)
    } else {
        let mut refs = store.references_at_round(round - 2);
        refs.extend(store.references_at_round(round - 1));
        refs
    };
    let mut new_blocks = Vec::new();
    for a in 0..n {
        let block = make_block_with_txs(
            AuthorityIndex(a as u64),
            round,
            prev_refs.clone(),
            transactions_per_block,
        );
        new_blocks.push(block);
    }
    for core in cores {
        core.add_blocks(new_blocks.clone());
    }
    new_blocks
}

/// Drain commit channel and return (blocks_committed, transactions_committed).
fn drain_commits(rx: &mut mpsc::UnboundedReceiver<mysticeti_core::CommittedSubDag>) -> (u64, u64) {
    let mut blocks = 0u64;
    let mut transactions = 0u64;
    while let Ok(subdag) = rx.try_recv() {
        blocks += subdag.blocks.len() as u64;
        for b in &subdag.blocks {
            transactions += b
                .statements
                .iter()
                .filter(|s| matches!(s, BaseStatement::Share(_)))
                .count() as u64;
        }
    }
    (blocks, transactions)
}

fn bench_tps(c: &mut Criterion) {
    const MEASURE_SECS: u64 = 2;
    const N_VALIDATORS: usize = 4;

    let mut group = c.benchmark_group("tps");
    group.sample_size(10);
    group.warm_up_time(Duration::from_secs(1));

    // Multi-proposer: 1, 2, and 4 leaders per wave (4 validators => up to 4 leaders)
    for number_of_leaders in [1u32, 2, 4] {
        for transactions_per_block in [1, 10, 100] {
            group.bench_with_input(
                criterion::BenchmarkId::new(
                    format!("4_validators_{}_leaders", number_of_leaders),
                    format!("{}_tx_per_block", transactions_per_block),
                ),
                &(number_of_leaders, transactions_per_block),
                |b, &(num_leaders, tx_per_block)| {
                    b.iter(|| {
                        let committee = make_committee(N_VALIDATORS);
                        let params = make_params(num_leaders);
                        let cores: Vec<_> = (0..N_VALIDATORS)
                            .map(|i| {
                                Core::new(
                                    params.clone(),
                                    committee.clone(),
                                    AuthorityIndex(i as u64),
                                    None,
                                    NoOpBlockHandler,
                                )
                            })
                            .collect();
                        let (tx, mut rx) = mpsc::unbounded_channel();
                        let observer = Arc::new(SimpleCommitObserver::new(tx));

                        // Warmup: run a few rounds so we have at least one full wave (0,1,2) then sync all cores
                        for _ in 0..3 {
                            run_round(&cores, tx_per_block);
                        }
                        for core in &cores {
                            core.try_commit(observer.as_ref());
                        }
                        let _ = drain_commits(&mut rx);

                        // Measured run
                        let measure_end = Instant::now() + Duration::from_secs(MEASURE_SECS);
                        let mut total_blocks = 0u64;
                        let mut total_tx = 0u64;
                        while Instant::now() < measure_end {
                            run_round(&cores, tx_per_block);
                            for core in &cores {
                                core.try_commit(observer.as_ref());
                            }
                            let (b, t) = drain_commits(&mut rx);
                            total_blocks += b;
                            total_tx += t;
                        }

                        // Report TPS (transactions per second) for this sample
                        let elapsed_secs = MEASURE_SECS as f64;
                        let tps = total_tx as f64 / elapsed_secs;
                        let bps = total_blocks as f64 / elapsed_secs;
                        eprintln!(
                            "  {} leaders, {} tx/block: {:.0} TPS, {:.0} blocks/sec (in {:.1}s)",
                            num_leaders, tx_per_block, tps, bps, elapsed_secs
                        );
                        (total_blocks, total_tx)
                    })
                },
            );
        }
    }
    group.finish();
}

criterion_group!(benches, bench_tps);
criterion_main!(benches);
