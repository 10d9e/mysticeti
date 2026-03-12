//! Mysticeti Docker client: submit random transactions to a set of validator nodes.
//!
//! This binary is intended to run inside the same Docker network as the validators.
//! It connects over TCP to simple submission ports exposed by the validators (one
//! per node) and sends newline-delimited transaction payloads.
//!
//! Environment:
//! - `MYSTICETI_CLIENT_TARGETS`: comma-separated list of host:port (e.g. `validator0:7000,validator1:7001`).
//! - `MYSTICETI_CLIENT_DURATION_SECS`: how long to run (default: 60).
//! - `MYSTICETI_CLIENT_RATE_PER_SEC`: approximate total target transactions per second (default: 100).
//! - `MYSTICETI_CLIENT_WAIT_SECS`: max seconds to wait for validators to be reachable at startup (default: 30).
//! - `MYSTICETI_CLIENT_WORKERS`: number of parallel send workers (default: 1).

use std::env;
use std::time::{Duration, Instant};

// rand is only used for simple payload content in this client; target selection
// uses a timestamp-based heuristic to avoid non-Send RNG state across awaits.
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

struct ConnectionPool {
    targets: Vec<String>,
    conns: Vec<Option<TcpStream>>,
}

impl ConnectionPool {
    fn new(targets: Vec<String>) -> Self {
        let mut conns = Vec::with_capacity(targets.len());
        for _ in 0..targets.len() {
            conns.push(None);
        }
        Self { targets, conns }
    }

    fn len(&self) -> usize {
        self.targets.len()
    }

    fn target(&self, idx: usize) -> &str {
        &self.targets[idx]
    }

    fn take_conn(&mut self, idx: usize) -> Option<TcpStream> {
        self.conns[idx].take()
    }

    fn put_conn(&mut self, idx: usize, conn: TcpStream) {
        self.conns[idx] = Some(conn);
    }
}

/// Wait until at least one target is connectable (validators may start after the client container).
async fn wait_for_validators(targets: &[String], max_wait: Duration) {
    let deadline = Instant::now() + max_wait;
    while Instant::now() < deadline {
        for target in targets {
            if TcpStream::connect(target).await.is_ok() {
                println!("mysticeti-client: validator {} reachable", target);
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    eprintln!(
        "mysticeti-client: no validator became reachable within {:?}, continuing anyway",
        max_wait
    );
}

async fn run_worker(
    worker_id: usize,
    targets: Vec<String>,
    duration_secs: u64,
    per_worker_rate: u64,
) -> u64 {
    let mut pool = ConnectionPool::new(targets);
    let interval = if per_worker_rate == 0 {
        Duration::from_secs(duration_secs)
    } else {
        Duration::from_secs_f64(1.0 / per_worker_rate as f64)
    };

    let start = Instant::now();
    let end = start + Duration::from_secs(duration_secs);
    let mut sent: u64 = 0;

    while Instant::now() < end {
        let idx = if pool.len() == 1 {
            0
        } else {
            // pseudo-random selection without keeping non-Send RNG state across awaits
            (Instant::now().elapsed().subsec_nanos() as usize) % pool.len()
        };
        let target = pool.target(idx).to_string();
        let payload = format!("w{}-tx-{}-{}", worker_id, sent, Instant::now().elapsed().as_millis());
        // Get or establish a connection for this target.
        let mut stream = match pool.take_conn(idx) {
            Some(s) => s,
            None => match TcpStream::connect(&target).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("[worker {}] failed to connect to {}: {}", worker_id, target, e);
                    tokio::time::sleep(interval).await;
                    continue;
                }
            },
        };
        let line = format!("{}\n", payload);
        if let Err(e) = stream.write_all(line.as_bytes()).await {
            eprintln!("[worker {}] failed to write to {}: {}", worker_id, target, e);
        } else {
            sent += 1;
        }
        // Put the connection back into the pool for reuse.
        pool.put_conn(idx, stream);
        tokio::time::sleep(interval).await;
    }

    sent
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let targets_env =
        env::var("MYSTICETI_CLIENT_TARGETS").unwrap_or_else(|_| "validator0:7000".to_string());
    let targets_vec: Vec<String> = targets_env
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if targets_vec.is_empty() {
        anyhow::bail!("no targets configured in MYSTICETI_CLIENT_TARGETS");
    }

    let duration_secs: u64 = env::var("MYSTICETI_CLIENT_DURATION_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let total_rate_per_sec: u64 = env::var("MYSTICETI_CLIENT_RATE_PER_SEC")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(100);
    let wait_secs: u64 = env::var("MYSTICETI_CLIENT_WAIT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(30);
    let workers: usize = env::var("MYSTICETI_CLIENT_WORKERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1)
        .max(1);

    let per_worker_rate = std::cmp::max(1, total_rate_per_sec / workers as u64);

    println!(
        "mysticeti-client: sending to {:?} for {}s at ~{} tx/s ({} workers, ~{} tx/s each)",
        targets_vec, duration_secs, total_rate_per_sec, workers, per_worker_rate
    );

    wait_for_validators(&targets_vec, Duration::from_secs(wait_secs)).await;

    let mut handles = Vec::new();
    for worker_id in 0..workers {
        let t = targets_vec.clone();
        handles.push(tokio::spawn(run_worker(
            worker_id,
            t,
            duration_secs,
            per_worker_rate,
        )));
    }

    let mut total_sent: u64 = 0;
    for handle in handles {
        match handle.await {
            Ok(worker_sent) => {
                total_sent += worker_sent;
            }
            Err(e) => {
                eprintln!("worker task join error: {}", e);
            }
        }
    }

    println!(
        "mysticeti-client: sent {} transactions in {}s (approx {:.1} tx/s)",
        total_sent,
        duration_secs,
        total_sent as f64 / duration_secs as f64
    );

    Ok(())
}

