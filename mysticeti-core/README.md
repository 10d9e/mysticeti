# mysticeti-core

Transport-agnostic core of the Mysticeti DAG consensus protocol.

- **Types**: `StatementBlock`, `BlockReference`, `BaseStatement`, committee and round types.
- **Committee**: Validator set, stake, quorum/validity thresholds, leader election, genesis blocks.
- **Block store / manager**: Causal ordering, inclusion rules, validation.
- **Consensus**: Wave-based base committer, universal committer, linearizer → `CommittedSubDag`.
- **Core loop**: `add_blocks`, `try_new_block`, `try_commit`; traits for `BlockHandler` and `CommitObserver`.

No network or Iroh dependency; suitable for tests with in-memory or simulated transport.
