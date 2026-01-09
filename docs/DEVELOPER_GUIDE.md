# realtime-core Developer Guide

**Contributing, testing, and benchmarking deterministic timing primitives**

## Table of Contents

1. [Development Setup](#development-setup)
2. [Project Structure](#project-structure)
3. [Testing Strategy](#testing-strategy)
4. [Performance Benchmarking](#performance-benchmarking)
5. [Release Process](#release-process)
6. [Code Review Guidelines](#code-review-guidelines)
7. [Architecture Decisions](#architecture-decisions)

---

## Development Setup

### Prerequisites

```bash
# Install Rust 1.75+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup default stable

# Install development tools
sudo apt install build-essential clang protobuf-compiler

# Install testing tools
sudo apt install rt-tests cyclictest

# Optional: Install PREEMPT_RT kernel for testing
# See USER_GUIDE.md for instructions
```

### Clone and Build

```bash
# Clone repository
git clone https://github.com/equilibrium-tokens/realtime-core.git
cd realtime-core

# Build with default features
cargo build

# Build with all features (PREEMPT_RT + io_uring)
cargo build --features full

# Run tests
cargo test

# Run with logging enabled
RUST_LOG=debug cargo test -- --nocapture
```

### Development Workflow

```bash
# 1. Create feature branch
git checkout -b feature/your-feature-name

# 2. Make changes and test
cargo test
cargo clippy
cargo fmt

# 3. Run integration tests (requires real-time hardware)
cargo test --test integration_tests

# 4. Run benchmarks
cargo bench

# 5. Commit with conventional commits
git commit -m "feat: add support for io_uring timeout operations"

# 6. Push and create PR
git push origin feature/your-feature-name
```

### Pre-commit Hooks

Install pre-commit hooks for automated checks:

```bash
# Install pre-commit
pip install pre-commit

# Install hooks
pre-commit install

# Run manually
pre-commit run --all-files
```

**`.pre-commit-config.yaml`**:
```yaml
repos:
  - repo: local
    hooks:
      - id: fmt
        name: rustfmt
        entry: cargo fmt -- --check
        language: system
        files: \.rs$
      - id: clippy
        name: clippy
        entry: cargo clippy -- -D warnings
        language: system
        files: \.rs$
      - id: test
        name: cargo test
        entry: cargo test
        language: system
        pass_filenames: false
```

---

## Project Structure

```
realtime-core/
├── Cargo.toml              # Project manifest
├── README.md               # Project overview
├── LICENSE                 # MIT license
├── docs/
│   ├── ARCHITECTURE.md     # System architecture
│   ├── USER_GUIDE.md       # User documentation
│   ├── DEVELOPER_GUIDE.md  # This file
│   └── RESEARCH.md         # Research findings
├── src/
│   ├── lib.rs              # Library root
│   ├── timer.rs            # Timer implementation
│   ├── scheduler.rs        # Scheduler implementation
│   ├── executor.rs         # RealtimeExecutor implementation
│   ├── backend/
│   │   ├── mod.rs          # Backend implementations
│   │   ├── timerfd.rs      # timerfd backend
│   │   ├── io_uring.rs     # io_uring backend
│   │   └── mock.rs         # Mock backend (testing)
│   ├── metrics.rs          # Jitter metrics and stats
│   └── error.rs            # Error types
├── examples/
│   ├── basic_timer.rs      # Basic timer usage
│   ├── rate_control.rs     # Rate control example
│   ├── cpu_isolation.rs    # CPU isolation example
│   ├── io_uring_timer.rs   # io_uring example
│   └── equilibrium_integration.rs
├── benches/
│   ├── timer_jitter.rs     # Jitter benchmark
│   └── scheduling_overhead.rs
└── tests/
    ├── integration_tests.rs
    └── mock_tests.rs
```

### Module Organization

**`src/lib.rs`** - Public API
```rust
//! realtime-core: Deterministic timing primitives
//!
//! # Example
//! ```rust
//! use realtime_core::Timer;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let mut timer = Timer::new(2.0)?;
//!     loop {
//!         timer.wait_for_tick().await?;
//!         // Your code here
//!     }
//! }
//! ```

pub mod timer;
pub mod scheduler;
pub mod executor;

// Re-export common types
pub use timer::Timer;
pub use scheduler::{Scheduler, SchedulingPolicy};
pub use executor::{RealtimeExecutor, ExecutorConfig};

// Error types
pub use error::{TimerError, SchedulerError, ExecutorError};

// Metrics
pub use metrics::{JitterMeasurement, JitterStats};

// Timeless math functions
pub use metrics::rate_to_interval_ns;
```

---

## Testing Strategy

### Philosophy

**Test without real-time hardware, verify with real-time hardware.**

1. **Unit tests**: Mock timer, deterministic, run on any system
2. **Integration tests**: Real timers, require PREEMPT_RT
3. **Benchmark tests**: Measure jitter, require PREEMPT_RT
4. **Property tests**: Verify invariants with proptest

### Unit Tests (Mock Timer)

**`tests/mock_tests.rs`**:
```rust
use realtime_core::{Timer, rate_to_interval_ns};
use std::time::Duration;

#[test]
fn test_timeless_rate_interval_math() {
    // This is physics: it should never change
    assert_eq!(rate_to_interval_ns(1.0), 1_000_000_000);
    assert_eq!(rate_to_interval_ns(2.0), 500_000_000);
    assert_eq!(rate_to_interval_ns(10.0), 100_000_000);
}

#[tokio::test]
async fn test_mock_timer_creates_successfully() {
    // Mock timer should work on any system
    let timer = Timer::new(2.0);
    assert!(timer.is_ok());

    let timer = timer.unwrap();
    assert_eq!(timer.rate(), 2.0);
    assert_eq!(timer.interval_ns(), 500_000_000);
}

#[test]
fn test_invalid_rate_rejected() {
    // Rate must be positive
    let timer = Timer::new(0.0);
    assert!(timer.is_err());

    let timer = Timer::new(-1.0);
    assert!(timer.is_err());

    let timer = Timer::new(f64::INFINITY);
    assert!(timer.is_err());
}

#[tokio::test]
async fn test_mock_timer_ticks() {
    let mut timer = Timer::new(10.0).unwrap();  // 10 Hz = 100ms interval

    let start = std::time::Instant::now();
    for _ in 0..10 {
        timer.wait_for_tick().await.unwrap();
    }
    let elapsed = start.elapsed();

    // Mock timer should tick immediately (no real delay)
    assert!(elapsed < Duration::from_millis(10));
}
```

### Integration Tests (Real Timer)

**`tests/integration_tests.rs`**:
```rust
#![cfg(feature = "preempt-rt")]

use realtime_core::{Timer, JitterMeasurement, JitterStats};
use std::time::Instant;

#[tokio::test]
#[ignore]  // Only run on real-time systems
async fn test_real_timer_10hz_jitter() {
    let mut timer = Timer::new(10.0).unwrap();  // 10 Hz
    let mut measurements = Vec::new();
    let mut last_tick = Instant::now();

    // Collect 1000 measurements
    for _ in 0..1000 {
        timer.wait_for_tick().await.unwrap();
        let now = Instant::now();
        let actual_interval = now.duration_since(last_tick).as_nanos() as u64;

        measurements.push(JitterMeasurement::new(
            timer.interval_ns(),
            actual_interval,
        ));

        last_tick = now;
    }

    let stats = JitterStats::from_measurements(&measurements);

    // On PREEMPT_RT, P99 should be <2ms
    assert!(
        stats.p99_ns < 2_000_000,
        "P99 jitter {}ns exceeds 2ms target",
        stats.p99_ns
    );

    // P50 should be <1ms
    assert!(
        stats.p50_ns < 1_000_000,
        "P50 jitter {}ns exceeds 1ms target",
        stats.p50_ns
    );

    println!("Jitter: P50={}µs, P95={}µs, P99={}µs",
        stats.p50_ns / 1000,
        stats.p95_ns / 1000,
        stats.p99_ns / 1000,
    );
}

#[tokio::test]
#[ignore]
async fn test_timer_drift_over_time() {
    let mut timer = Timer::new(1.0).unwrap();  // 1 Hz
    let start = Instant::now();

    // Run for 100 seconds
    for i in 0..100 {
        timer.wait_for_tick().await.unwrap();

        let elapsed = start.elapsed().as_secs_f64();
        let expected = i as f64;

        let drift_ms = (elapsed - expected).abs() * 1000.0;

        // Drift should be <10ms over 100 seconds
        assert!(
            drift_ms < 10.0,
            "Drift {}ms at tick {}",
            drift_ms, i
        );
    }
}
```

### Property Tests

**`tests/prop_tests.rs`**:
```rust
use proptest::prelude::*;
use realtime_core::{Timer, rate_to_interval_ns};

proptest! {
    #[test]
    fn test_rate_interval_inverse(rate in 0.1..1000.0) {
        let interval = rate_to_interval_ns(rate);
        let recovered_rate = 1_000_000_000.0 / interval as f64;

        // Should recover original rate within 0.1% tolerance
        prop_assert!((recovered_rate - rate).abs() / rate < 0.001);
    }

    #[test]
    fn test_rate_interval_roundtrip(rate in 0.1..1000.0) {
        let interval = rate_to_interval_ns(rate);
        let interval2 = rate_to_interval_ns(rate);

        // Same rate should always produce same interval
        prop_assert_eq!(interval, interval2);
    }
}
```

### Running Tests

```bash
# Unit tests (no PREEMPT_RT required)
cargo test

# Integration tests (requires PREEMPT_RT)
cargo test --test integration_tests -- --ignored

# Property tests
cargo test --test prop_tests

# With output
cargo test -- --nocapture

# Specific test
cargo test test_timeless_rate_interval_math

# Run tests with backtrace on panic
RUST_BACKTRACE=1 cargo test
```

---

## Performance Benchmarking

### Benchmark Design

**`benches/timer_jitter.rs`**:
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};
use realtime_core::{Timer, JitterMeasurement, JitterStats};
use std::time::{Duration, Instant};

fn bench_timer_jitter(c: &mut Criterion) {
    let mut group = c.benchmark_group("timer_jitter");

    for rate in [1.0, 2.0, 10.0, 100.0].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(rate), rate, |b, &rate| {
            let rt = tokio::runtime::Runtime::new().unwrap();

            b.to_async(&rt).iter(|| {
                let mut timer = Timer::new(rate).unwrap();

                async move {
                    let mut measurements = Vec::new();
                    let mut last_tick = Instant::now();

                    // Measure 100 ticks
                    for _ in 0..100 {
                        black_box(timer.wait_for_tick().await.unwrap());
                        let now = Instant::now();
                        let actual_interval = now.duration_since(last_tick).as_nanos() as u64;

                        measurements.push(JitterMeasurement::new(
                            timer.interval_ns(),
                            actual_interval,
                        ));

                        last_tick = now;
                    }

                    let stats = JitterStats::from_measurements(&measurements);
                    (stats.p50_ns, stats.p99_ns)
                }
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_timer_jitter);
criterion_main!(benches);
```

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Run specific benchmark
cargo bench --bench timer_jitter

# Save baseline
cargo bench -- --save-baseline main

# Compare with baseline
cargo bench -- --baseline main

# Generate flamegraph
cargo bench --bench timer_jitter -- --profile-time=10
```

### Benchmark Results Interpretation

**Expected Results (PREEMPT_RT + io_uring)**:

```
timer_jitter/1.0
                        time:   [101.23 ms 102.45 ms 103.89 ms]
                        change: [-2.3% -1.1% +0.5%] (p = 0.23 > 0.05)
                        No change in performance detected.

timer_jitter/2.0
                        time:   [50.12 ms 50.67 ms 51.23 ms]
                        change: [-1.8% -0.9% +0.3%] (p = 0.31 > 0.05)
                        No change in performance detected.

P50 Jitter: ~300µs
P99 Jitter: ~1.2ms
```

**Expected Results (standard Linux)**:

```
P50 Jitter: ~2ms
P99 Jitter: ~8ms
```

### Continuous Benchmarking

Set up CI to track performance over time:

**`.github/workflows/benchmark.yml`**:
```yaml
name: Benchmark

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Install Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable

      - name: Run benchmarks
        run: cargo bench -- --save-baseline main

      - name: Store benchmark result
        uses: benchmark-action/github-action-benchmark@v1
        with:
          tool: 'cargo'
          output-file-path: target/criterion/results.json
```

---

## Release Process

### Versioning

Follow Semantic Versioning 2.0.0:
- **MAJOR**: Breaking API changes
- **MINOR**: New features, backwards compatible
- **PATCH**: Bug fixes, backwards compatible

### Pre-Release Checklist

- [ ] All tests pass: `cargo test`
- [ ] Benchmarks run: `cargo bench`
- [ ] Documentation updated: `cargo doc`
- [ ] Changelog updated
- [ ] Version bumped in `Cargo.toml`
- [ ] Tagged commit: `git tag -a v0.x.y -m "Release v0.x.y"`

### Release Steps

```bash
# 1. Update version in Cargo.toml
vim Cargo.toml  # Bump version

# 2. Update CHANGELOG.md
vim CHANGELOG.md

# 3. Commit changes
git commit -am "Release v0.1.0"

# 4. Create tag
git tag -a v0.1.0 -m "Release v0.1.0"

# 5. Push to GitHub
git push origin main --tags

# 6. Publish to crates.io
cargo publish --dry-run  # Test
cargo publish           # Live

# 7. Create GitHub release
gh release create v0.1.0 --notes "Release v0.1.0: ..."
```

### Release Notes Template

```markdown
# Release v0.1.0

## Features
- Add io_uring backend for 30-50% jitter reduction
- Support SCHED_DEADLINE scheduling
- Add Python bindings via PyO3

## Bug Fixes
- Fix timer drift over long periods
- Correct CPU affinity on multi-core systems

## Performance
- P99 latency: 2ms → 1.2ms (40% improvement)
- CPU usage: 15% → 8% (47% reduction)

## Breaking Changes
- Removed `Timer::with_interval()` in favor of `Timer::new()`

## Migration Guide
```rust
// Old
let timer = Timer::with_interval(Duration::from_millis(500))?;

// New
let timer = Timer::new(2.0)?;  // 2 Hz = 500ms interval
```
```

---

## Code Review Guidelines

### Review Checklist

**Functionality**
- [ ] Code solves the stated problem
- [ ] Edge cases handled (zero rate, overflow, etc.)
- [ ] Error paths tested

**Performance**
- [ ] No allocations in timing-critical paths
- [ ] Lock-free data structures where possible
- [ ] Benchmarks show no regression

**Documentation**
- [ ] Public API documented
- [ ] Examples provided
- [ ] Architectural decisions explained

**Testing**
- [ ] Unit tests cover new code
- [ ] Integration tests pass on PREEMPT_RT
- [ ] Property tests verify invariants

### Example Review

**Pull Request: "Add io_uring timeout support"**

```rust
// ✅ Good: Clear documentation, error handling
/// Create io_uring timeout operation
///
/// # Errors
/// Returns `TimerError::Io` if io_uring syscall fails
fn io_uring_timeout_new(interval: Duration) -> Result<TimerBackend, TimerError> {
    let uring = IoUring::new(8)
        .map_err(|e| TimerError::Io(e.into()))?;

    // ... implementation
}

// ❌ Bad: No error handling, unclear semantics
fn io_uring_timeout_new(interval: Duration) -> TimerBackend {
    let uring = IoUring::new(8).unwrap();
    // ... implementation
}
```

---

## Architecture Decisions

### Decision: Use Tokio for Async Runtime

**Context**: Need async executor for real-time tasks

**Options**:
1. Tokio (mature, widely used)
2. async-std (simpler API)
3. Custom executor (maximum control)

**Decision**: Tokio

**Rationale**:
- Maturity and ecosystem (tokio-uring)
- Excellent performance with work-stealing scheduler
- Proven in production real-time systems
- Integration with equilibrium-tokens (already uses Tokio)

**Consequences**:
- Positive: Leverages existing ecosystem
- Positive: Easy integration with tokio-uring
- Negative: Larger dependency tree
- Negative: Less control than custom executor

### Decision: Support Multiple Timer Backends

**Context**: Need to support systems without PREEMPT_RT

**Options**:
1. io_uring only (modern Linux)
2. timerfd only (POSIX standard)
3. Multiple backends with feature flags

**Decision**: Multiple backends with feature flags

**Rationale**:
- **Maximum compatibility**: Support legacy systems
- **Optimal performance**: Use best available backend
- **Testing**: Mock backend for unit tests
- **Future-proof**: Easy to add new backends

**Consequences**:
- Positive: Works on any Linux 5.1+
- Positive: Optimal performance on PREEMPT_RT
- Negative: More complex codebase
- Negative: Feature flag combinatorics

### Decision: Use Rust for Core Library

**Context**: Need low-level timing primitives

**Options**:
1. Rust (this project)
2. C with FFI bindings
3. Go with cgo

**Decision**: Rust

**Rationale**:
- **Memory safety**: No use-after-free in timing code
- **Low-level control**: Direct syscall access
- **Zero-cost abstractions**: High-level API, low-level performance
- **Async support**: Native async/await
- **FFI**: Easy to expose to Python (PyO3), Go (cgo), TypeScript (neon)

**Consequences**:
- Positive: Memory safety without GC
- Positive: Excellent compile-time guarantees
- Positive: Easy cross-language integration
- Negative: Steeper learning curve for contributors
- Negative: Longer compile times

---

## Contributing

### First-Time Contributors

1. Read this guide
2. Set up development environment
3. Find "good first issue" on GitHub
4. Claim issue (leave comment)
5. Create branch
6. Implement with tests
7. Submit PR

### Regular Contributors

1. Review open PRs
2. Help in discussions
3. mentor first-time contributors
4. Write documentation
5. Improve benchmarks

### Maintainer Responsibilities

1. Review and merge PRs
2. Tag releases
3. Respond to security issues
4. Update documentation
5. Manage roadmap

---

## Getting Help

- **Documentation**: See `docs/` directory
- **Issues**: GitHub Issues
- **Discussions**: GitHub Discussions
- **Email**: maintainers@equilibrium-tokens.org

---

**Thank you for contributing to realtime-core! The grammar endures.**
