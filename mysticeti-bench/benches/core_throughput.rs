//! Benchmarks for mysticeti-core: add_blocks, try_commit, linearizer.

use criterion::{black_box, criterion_group, criterion_main, BatchSize, Criterion, SamplingMode};
use mysticeti_core::{
    Authority, AuthorityIndex, BlockDigest, BlockSignature, Committee, Core, Data, EpochNumber,
    NoOpBlockHandler, Parameters, SimpleCommitObserver, StatementBlock,
};
use std::sync::Arc;
use tokio::sync::mpsc;

fn make_committee(n: u8) -> Committee {
    let mut authorities = Vec::new();
    for i in 0..n {
        authorities.push(Authority {
            stake: 1,
            public_key: mysticeti_core::PublicKey([i; 32]),
            hostname: format!("v{}", i),
        });
    }
    Committee {
        authorities,
        epoch: EpochNumber(0),
        validity_threshold: (n as u64) / 2 + 1,
        quorum_threshold: 2 * (n as u64) / 3 + 1,
    }
}

fn make_params() -> Parameters {
    Parameters {
        wave_length: 3,
        number_of_leaders: 1,
        ..Default::default()
    }
}

fn make_block(creator: AuthorityIndex, round: u64, includes: Vec<mysticeti_core::BlockReference>) -> Data<StatementBlock> {
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

fn bench_add_blocks(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_add_blocks");
    group.sampling_mode(SamplingMode::Flat);
    group.sample_size(50);

    for size in [10, 50, 100, 200].iter() {
        group.bench_function(format!("batch_{}", size), |b| {
            b.iter_batched(
                || {
                    let committee = make_committee(4);
                    let params = make_params();
                    let core = Core::new(
                        params,
                        committee,
                        AuthorityIndex(0),
                        None,
                        NoOpBlockHandler,
                    );
                    let blocks: Vec<Data<StatementBlock>> = (0..*size)
                        .map(|i| {
                            make_block(
                                AuthorityIndex((i % 4) as u64),
                                i as u64 / 4,
                                vec![],
                            )
                        })
                        .collect();
                    (core, blocks)
                },
                |(core, blocks)| {
                    black_box(core.add_blocks(blocks));
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

fn bench_try_commit(c: &mut Criterion) {
    let (tx, _rx) = mpsc::unbounded_channel();
    let observer = Arc::new(SimpleCommitObserver::new(tx));

    let mut group = c.benchmark_group("core_try_commit");
    group.sampling_mode(SamplingMode::Flat);

    group.bench_function("after_rounds", |b| {
        b.iter_batched(
            || {
                let committee = make_committee(4);
                let params = make_params();
                let core = Core::new(
                    params,
                    committee,
                    AuthorityIndex(0),
                    None,
                    NoOpBlockHandler,
                );
                // Pre-fill: round 0 (4 blocks), then round 1 with includes pointing at round 0.
                let r0: Vec<_> = (0..4u64)
                    .map(|a| make_block(AuthorityIndex(a), 0, vec![]))
                    .collect();
                core.add_blocks(r0.clone());
                let refs0: Vec<_> = r0.iter().map(|b| b.reference()).collect();
                let r1: Vec<_> = (0..4u64)
                    .map(|a| make_block(AuthorityIndex(a), 1, refs0.clone()))
                    .collect();
                core.add_blocks(r1);
                core
            },
            |core| {
                core.try_commit(observer.as_ref());
            },
            BatchSize::SmallInput,
        )
    });
    group.finish();
}

fn bench_linearizer(c: &mut Criterion) {
    use mysticeti_core::Linearizer;

    let mut group = c.benchmark_group("linearizer");
    group.sampling_mode(SamplingMode::Flat);

    for n in [10, 50, 100].iter() {
        group.bench_function(format!("n_blocks_{}", n), |b| {
            b.iter_batched(
                || {
                    let blocks: Vec<Data<StatementBlock>> = (0..*n)
                        .map(|i| {
                            let round = i / 4;
                            let authority = i % 4;
                            let includes: Vec<_> = if round > 0 {
                                (0..4)
                                    .map(|a| mysticeti_core::BlockReference {
                                        authority: AuthorityIndex(a),
                                        round: round - 1,
                                        digest: BlockDigest([0u8; 32]),
                                    })
                                    .collect()
                            } else {
                                vec![]
                            };
                            make_block(AuthorityIndex(authority as u64), round as u64, includes)
                        })
                        .collect();
                    (Linearizer::new(), blocks)
                },
                |(lin, blocks)| {
                    black_box(lin.linearize(blocks));
                },
                BatchSize::SmallInput,
            )
        });
    }
    group.finish();
}

criterion_group!(benches, bench_add_blocks, bench_try_commit, bench_linearizer);
criterion_main!(benches);
