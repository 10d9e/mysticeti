# mysticeti-bench

Benchmarks and multi-node harness for Mysticeti.

## Benchmarks

- **core_throughput**: Core hot paths
  - `core_add_blocks`: Throughput of `Core::add_blocks` for batch sizes 10, 50, 100, 200.
  - `core_try_commit`: Cost of `Core::try_commit` with a pre-filled store (rounds 0–1).
  - `linearizer`: Cost of `Linearizer::linearize` for 10, 50, 100 blocks.

- **multi_node**: In-process multi-validator scenario
  - `4_validators_until_commit`: Four cores, simulated round-by-round block exchange, run 20 rounds and measure elapsed time.

## Run

```bash
cargo bench -p mysticeti-bench
cargo bench -p mysticeti-bench -- core_throughput
cargo bench -p mysticeti-bench -- multi_node
```

Results and HTML reports are written to `target/criterion/`.
