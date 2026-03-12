# Mysticeti over Iroh

A Rust implementation of the [Mysticeti](https://github.com/MystenLabs/mysticeti) DAG consensus protocol with [Iroh](https://github.com/n0-computer/iroh) as the P2P networking backbone.

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

## Benchmarks

The **mysticeti-bench** crate provides Criterion benchmarks for core throughput (`add_blocks`, `try_commit`, linearizer) and an in-process multi-node scenario (4 validators, simulated rounds).

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
