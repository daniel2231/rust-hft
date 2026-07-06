# CLAUDE.md

Guidance for Claude Code (and other contributors) when working on this repo. This project is a
low-latency crypto HFT system in Rust — performance and correctness on the hot path matter more
than idiomatic-but-slow code. The rules below are distilled from general HFT engineering practice
(see https://lucasbardella.com/coding/2025/rust-for-hft) applied to this codebase's actual
architecture.

## Hot path discipline

The hot path is: WebSocket message → `ingestion` → `orderbook` update → `strategy` evaluation →
`risk` check → `execution`. Anything on this path should be treated as latency-sensitive.

- **Minimize memory operations.** Arithmetic is cheap, memory access is not. Prefer stack
  allocation and fixed-size/pre-allocated buffers over heap allocation in the orderbook/strategy/
  risk code. Before adding a `.clone()` on the hot path, ask whether a reference, `Copy`, or
  restructuring the ownership would avoid it. `OrderbookSnapshot` clones (e.g. in
  `src/dashboard/mod.rs`) are acceptable off the hot path (dashboard push is 1/sec) but should not
  be introduced inside `orderbook.rs` update logic or `strategy::on_orderbook`.
- **Minimize IO on the critical path.** No blocking disk/network IO inside orderbook update,
  strategy evaluation, or risk checks. Logging is fine (async, buffered via `tracing`), but avoid
  synchronous file writes or extra REST calls per event. Existing patterns to preserve: the
  kill-switch `HALT` file is polled by a background task in `main.rs` that updates a shared
  `AtomicBool` — risk checks and the dashboard read the atomic, never the filesystem; the
  paper-trading order log holds one file handle for the process lifetime (one `write` syscall
  per order, no open/close). Don't regress either back to per-event filesystem calls.
- Prefer bounded, pre-sized collections (`VecDeque::with_capacity`, fixed depth in
  `orderbook::depth_levels`) over unbounded growth.

## Threading model

This repo already follows the "few threads, minimal cross-thread chatter" principle:

- **Thread 1 (Tokio async runtime):** WebSocket ingestion, REST snapshot fetch, order execution,
  dashboard HTTP/WS server.
- **Thread 2 (OS thread):** Orderbook maintenance, strategy evaluation, risk checks.
- **Bridge:** `crossbeam-channel` between the async and sync worlds.

Keep it this way. Do not casually add new threads or new channels for M6 strategy work — every
additional thread boundary adds real inter-thread communication cost (~100ns+ per hop, more under
contention) and a new place for backpressure/ordering bugs. If a new stage is genuinely needed
(e.g. a dedicated logging thread), prefer a dedicated **SPSC** ring buffer over a general MPMC
channel — SPSC queues are lock-free and cheaper when there really is exactly one producer and one
consumer. `crossbeam-channel` (MPMC) is the right default only when there are actually multiple
producers/consumers; don't reach for it out of habit if a link is truly 1:1.

`SharedState` (`src/dashboard/mod.rs`) uses atomics + `parking_lot::RwLock` for the sync-thread →
dashboard bridge instead of a channel, since the dashboard only needs the *latest* state, not every
intermediate event — a good pattern to keep for "latest value" fan-out, as opposed to ordered
event streams which belong on a channel.

## CPU / processor affinity

Not currently implemented. If profiling shows scheduler-induced jitter (cache misses, inconsistent
tail latencies) on the orderbook/strategy thread, consider pinning it with
[`core_affinity`](https://crates.io/crates/core_affinity) rather than leaving it to the OS
scheduler. Check core layout with `lscpu` before deciding a pinning scheme (physical core vs
hyperthread sibling matters). This is a Phase 2 (cloud VM) concern more than Phase 1 (Mac Mini
local dev) — don't add pinning logic prematurely; measure first.

## Concrete guidance for future work (M6 strategy, Phase 2 deployment)

- When implementing real `Strategy` impls in `src/strategy/`, keep `on_orderbook` allocation-free
  where possible — it's called once per event, at ~10 events/sec/symbol today but potentially much
  higher under live multi-symbol load.
- Before adding a new dependency for hot-path work, check whether it's warranted:
  - Binary encode/decode on the hot path → prefer something like `bincode` over `serde_json`
    (JSON is fine for the dashboard/logging paths, which are not latency-critical).
  - Precise/cheap timing → a dedicated timing crate rather than repeated `std::time::Instant` math
    scattered around, if this becomes a pattern (see `src/metrics.rs::LatencyTracker` for the
    existing minimal approach — extend that rather than inventing a parallel mechanism).
  - Shared memory / cross-process IPC — only if a future design needs multi-process rather than
    multi-thread; not needed today.
- Don't introduce processor-affinity or SPSC-queue machinery speculatively for M6 — add it when a
  measured latency problem calls for it, and note the measurement in the PR description.

## General rule of thumb

When touching `src/orderbook.rs`, `src/strategy/`, `src/risk.rs`, or `src/execution.rs`: default to
the version with fewer allocations, fewer thread hops, and fewer syscalls, even if it's a few lines
longer. When touching `src/dashboard/`, `src/backtest/`, or logging: normal Rust ergonomics are
fine — these are not on the critical path.
