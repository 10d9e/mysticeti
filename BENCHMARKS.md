# Mysticeti Benchmarks

This document summarizes TPS (transactions per second) and blocks/sec as measured by the `mysticeti-bench` TPS benchmark on this machine. The benchmark runs 4 validators in-process with a fixed 2-second measure window after a short warmup.

## How to run

```bash
cargo bench -p mysticeti-bench --bench tps
```

All configurations use 4 validators and `wave_length = 3`. Results are from 10 samples per configuration.

## TPS and blocks/sec (this machine)

| Leaders | Tx/block | TPS (approx) | Blocks/sec (approx) |
|--------:|----------|---------------|----------------------|
| 1       | 1        | ~9,100        | ~9,100               |
| 1       | 10       | ~88,000       | ~8,800               |
| 1       | 100      | ~835,000      | ~8,350               |
| 2       | 1        | ~13,800       | ~13,800              |
| 2       | 10       | ~137,000      | ~13,700              |
| 2       | 100      | ~1,300,000    | ~13,000              |
| 4       | 1        | ~19,500       | ~19,500              |
| 4       | 10       | ~195,000      | ~19,500              |
| 4       | 100      | ~1,920,000    | ~19,200              |

Multi-proposer (2 or 4 leaders per wave) increases committed blocks per second and thus TPS for a given tx/block size.

## Other benchmarks

- **Core throughput** (`add_blocks`, `try_commit`, linearizer):  
  `cargo bench -p mysticeti-bench -- core_throughput`
- **Multi-node** (4 validators, simulated rounds):  
  `cargo bench -p mysticeti-bench -- multi_node`

## Docker e2e throughput

The Dockerized 4-node + client demo reports end-to-end client tx/s (e.g. ~1,700 tx/s per client with 4 workers, ~3.4k tx/s with two client containers). That is limited by network, loop interval, and client/validator CPU rather than the in-process TPS numbers above.
