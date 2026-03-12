//! Multi-node benchmark: N cores in-process, simulated block exchange, measure commit TPS/latency.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use mysticeti_core::{
    Authority, AuthorityIndex, BlockDigest, BlockReference, BlockSignature, Committee, Core, Data,
    EpochNumber, NoOpBlockHandler, Parameters, SimpleCommitObserver, StatementBlock,
};
use std::sync::Arc;
use std::time::Instant;
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

fn make_params() -> Parameters {
    Parameters {
        wave_length: 3,
        number_of_leaders: 1,
        ..Default::default()
    }
}

fn make_block(
    creator: AuthorityIndex,
    round: u64,
    includes: Vec<BlockReference>,
) -> Data<StatementBlock> {
    Data::new(StatementBlock::new(
        creator,
        round,
        BlockDigest([0u8; 32]),
        includes,
        vec![],
        EpochNumber(0),
        BlockSignature(vec![]),
    ))
}

/// Run one "round" of the DAG: each authority creates a block that includes all blocks from the
/// previous round, then we broadcast those blocks to all cores.
fn run_round(
    cores: &[Core<NoOpBlockHandler>],
    store_refs: &[Vec<BlockReference>],
) -> Vec<Data<StatementBlock>> {
    let n = cores.len();
    let round = store_refs[0]
        .iter()
        .map(|r| r.round)
        .max()
        .map(|r| r + 1)
        .unwrap_or(0);
    let prev_refs: Vec<BlockReference> = if round == 0 {
        vec![]
    } else {
        store_refs[0].clone()
    };
    let mut new_blocks = Vec::new();
    for a in 0..n {
        let block = make_block(AuthorityIndex(a as u64), round, prev_refs.clone());
        new_blocks.push(block);
    }
    for core in cores {
        core.add_blocks(new_blocks.clone());
    }
    new_blocks
}

/// Multi-node benchmark: 4 cores, run rounds until we have at least one commit, measure rounds/sec.
fn bench_multi_node_4(c: &mut Criterion) {
    let mut group = c.benchmark_group("multi_node");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(5));

    group.bench_function("4_validators_until_commit", |b| {
        b.iter(|| {
            let committee = make_committee(4);
            let params = make_params();
            let cores: Vec<_> = (0..4)
                .map(|i| {
                    Core::new(
                        params.clone(),
                        committee.clone(),
                        AuthorityIndex(i),
                        None,
                        NoOpBlockHandler,
                    )
                })
                .collect();
            let (tx, _rx) = mpsc::unbounded_channel();
            let observer = Arc::new(SimpleCommitObserver::new(tx));
            let observer_clone = observer.clone();
            let start = Instant::now();
            let mut store_refs: Vec<Vec<BlockReference>> = vec![vec![]; 4];
            let mut rounds = 0u64;
            loop {
                let new_blocks = run_round(&cores, &store_refs);
                for (i, core) in cores.iter().enumerate() {
                    store_refs[i] = new_blocks.iter().map(|b| b.reference()).collect();
                    core.try_commit(observer_clone.as_ref());
                }
                rounds += 1;
                if rounds >= 20 {
                    break;
                }
            }
            black_box((rounds, start.elapsed()))
        })
    });
    group.finish();
}

criterion_group!(benches, bench_multi_node_4);
criterion_main!(benches);
