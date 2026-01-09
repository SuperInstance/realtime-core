# realtime-core - Quick Reference

**Deterministic timing primitives for real-time systems**

## Key Files

| File | Purpose | Lines |
|------|---------|-------|
| `README.md` | Project overview, quick start | ~180 |
| `Cargo.toml` | Dependencies, features, build config | ~100 |
| `docs/ARCHITECTURE.md` | Complete system architecture | ~1100 |
| `docs/USER_GUIDE.md` | Installation, usage, troubleshooting | ~650 |
| `docs/DEVELOPER_GUIDE.md` | Contributing, testing, benchmarking | ~600 |
| `DESIGN_SUMMARY.md` | This architecture design | ~400 |
| `examples/equilibrium_integration.rs` | Integration example | ~280 |

## Quick Start

```bash
# Add to Cargo.toml
[dependencies]
realtime-core = "0.1"

# Basic usage
use realtime_core::Timer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut timer = Timer::new(2.0)?;  // 2 Hz

    loop {
        timer.wait_for_tick().await?;
        // Your code here with <2ms jitter
    }
}
```

## Performance Targets

### On PREEMPT_RT (Linux 6.12+)
- **P99 Latency**: <2ms ✅
- **P95 Latency**: <1ms
- **P50 Latency**: <500µs
- **CPU Usage**: <10%

### On Standard Linux
- **P99 Latency**: 5-10ms (fallback)
- **CPU Usage**: 15-20%

## Core Components

### 1. Timer
```rust
let mut timer = Timer::new(2.0)?;  // 2 Hz
timer.wait_for_tick().await?;
```

### 2. Scheduler
```rust
let mut scheduler = Scheduler::new()?;
scheduler.set_cpu_affinity(vec![1])?;
scheduler.set_deadline(1_000_000, 500_000, 500_000_000)?;
scheduler.apply_to_current_thread()?;
```

### 3. RealtimeExecutor
```rust
let executor = RealtimeExecutor::with_config(ExecutorConfig {
    cpu_affinity: Some(vec![1]),
    scheduling_policy: SchedulingPolicy::Fifo(50),
    ..Default::default()
})?;
```

## Timeless Principle

```rust
// This is physics: time intervals are measured in nanoseconds
let interval_ns = (1_000_000_000.0 / target_rate_hz) as u64;
```

## Feature Flags

| Feature | Description | Required |
|---------|-------------|----------|
| `preempt-rt` | PREEMPT_RT support (Linux 6.12+) | Optional |
| `io-uring` | Zero-copy I/O (Linux 5.1+) | Optional |
| `timerfd` | POSIX fallback (default) | Yes |
| `deadline` | SCHED_DEADLINE support | Optional |
| `full` | All features for max performance | Optional |
| `testing` | Mock timer for tests | Optional |

## System Requirements

### Minimum
- Linux 5.1+ (for io_uring)
- Rust 1.75+
- CAP_SYS_NICE capability

### For <2ms Jitter
- **Linux 6.12+** with PREEMPT_RT
- **Isolated CPU cores** (`isolcpus=1`)
- **IRQ affinity** configured
- **SCHED_DEADLINE** support

## Installation

```bash
# Install PREEMPT_RT kernel (Ubuntu/Debian)
sudo apt install linux-image-rt-amd64

# Isolate CPU cores
sudo nano /etc/default/grub
# Add: isolcpus=1
sudo update-grub
sudo reboot

# Verify
uname -v | grep PREEMPT_RT
cat /sys/devices/system/cpu/isolated
```

## Testing

```bash
# Unit tests (no hardware required)
cargo test

# Integration tests (requires PREEMPT_RT)
cargo test --test integration_tests -- --ignored

# Benchmarks
cargo bench
```

## Documentation

```bash
# Generate documentation
cargo doc --open

# Read guides
less docs/ARCHITECTURE.md
less docs/USER_GUIDE.md
less docs/DEVELOPER_GUIDE.md
```

## Integration with Equilibrium Tokens

```rust
use equilibrium_tokens::RateEquilibrium;
use realtime_core::Timer;

let rate_surface = RateEquilibrium::new(2.0, 0.5, 5.0)?;
let mut timer = Timer::new(2.0)?;

loop {
    timer.wait_for_tick().await?;
    rate_surface.emit_token()?;
}
```

## Troubleshooting

### Jitter exceeds 2ms
```bash
# Check PREEMPT_RT
uname -v | grep PREEMPT_RT

# Check CPU isolation
cat /sys/devices/system/cpu/isolated

# Measure jitter
sudo cyclictest -p 80 -t1 -n -i 10000 -l 6000
```

### Permission denied
```bash
# Grant CAP_SYS_NICE
sudo setcap cap_sys_nice+ep /path/to/binary
```

## Key Metrics

| Metric | Formula | Example (2 Hz) |
|--------|---------|----------------|
| Interval (ns) | `10^9 / rate` | 500,000,000 ns |
| Interval (ms) | `1000 / rate` | 500 ms |
| Rate (Hz) | `10^9 / interval_ns` | 2.0 Hz |

## Architecture Decisions

1. **Tokio for async** - Mature, excellent performance
2. **Multiple backends** - timerfd, io_uring, mock
3. **Rust for core** - Memory safety + low-level control
4. **Feature flags** - Opt-in for PREEMPT_RT, io_uring

## Research Foundation

- **Linux PREEMPT_RT** (6.12+): Sub-100µs latency, 8.7× jitter reduction
- **io_uring**: 30-50% jitter reduction vs. timerfd
- **SCHED_DEADLINE**: Hard real-time scheduling
- **High-Resolution Timers**: 1 nanosecond resolution

## Next Steps

1. Read `docs/ARCHITECTURE.md` for complete design
2. Read `docs/USER_GUIDE.md` for setup instructions
3. Run `examples/equilibrium_integration.rs`
4. Implement core abstractions (Round 2)

## License

MIT License - see `LICENSE` for details.

---

**The grammar is eternal.**
