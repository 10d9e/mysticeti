# Mysticeti over Iroh

the paper Mysticeti: Low-Latency DAG Consensus with Fast Commit Path enabling reproducible results

A Rust implementation of the [Mysticeti](https://arxiv.org/abs/2310.14821) DAG consensus protocol with [Iroh](https://github.com/n0-computer/iroh) as the P2P networking backbone.

## Architecture

- **mysticeti-core**: Transport-agnostic protocol (types, committee, DAG store, wave-based consensus, commit observer, core loop). No Iroh or TCP.
- **mysticeti-net**: Iroh transport (QUIC, ALPN `mysticeti/0`), length-delimited bincode codec, protocol handler, peer identity, sync helpers.
- **mysticeti**: Binary that wires core + net: CLI, Endpoint, Router, inbox handler, core loop.

## Build and run

```bash
cargo build
cargo test -p mysticeti-core
cargo run -p mysticeti -- --authority-index 0
```

## Dockerized 4-node e2e example

A simple Docker-based end-to-end setup is provided with 4 validator nodes and a client
that submits random transactions across them.

Build and run:

```bash
# From the workspace root
docker compose build
docker compose up
```

This will:

- Build the `mysticeti` image using `mysticeti/Dockerfile`.
- Start 4 validators (`validator0`..`validator3`) using the shared
  `mysticeti/config/docker-committee.toml` config.
- Start a `client` container running the `mysticeti-client` binary, which connects
  to the validators on ports `7000`–`7003` and sends newline-delimited transactions
  for 60 seconds at ~100 tx/s by default.

Validator logs will show committed subdags; the client will print how many transactions
it sent during the run.

## Benchmarks

The **mysticeti-bench** crate provides Criterion benchmarks for core throughput (`add_blocks`, `try_commit`, linearizer) and an in-process multi-node scenario (4 validators, simulated rounds). See [BENCHMARKS.md](BENCHMARKS.md) for TPS and blocks/sec results (1/2/4 leaders, various tx-per-block sizes).

```bash
cargo bench -p mysticeti-bench
cargo bench -p mysticeti-bench -- core_throughput
cargo bench -p mysticeti-bench -- multi_node
cargo bench -p mysticeti-bench --bench tps
```

## Configuration

Use `--config` for a config file (optional). Without it, a 4-validator test committee is used. Options:

- `--authority-index`: This node’s index (default `0`).
- `--storage-dir`: Storage directory (default `./data`).

## License

Apache-2.0.
