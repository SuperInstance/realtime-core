# realtime-core

**Deterministic timing primitives for real-time systems in Rust**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![docs.rs](https://docs.rs/realtime-core/badge.svg)](https://docs.rs/realtime-core)

realtime-core provides hardware-level timing precision for Rust applications requiring sub-millisecond jitter guarantees. Built on Linux PREEMPT_RT, io_uring, and SCHED_DEADLINE, it delivers the deterministic timing needed by real-time systems like audio processing, high-frequency trading, and robotics.

## Key Features

- **<2ms jitter guarantee** on properly configured PREEMPT_RT systems
- **Hardware-level timing** using timerfd and io_uring backends
- **SCHED_DEADLINE support** for hard real-time scheduling
- **CPU isolation** to prevent scheduling interference
- **Graceful fallback** for systems without PREEMPT_RT
- **Async-first** with native tokio integration
- **Zero-copy networking** via io_uring for cross-language IPC

## Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
realtime-core = "0.1"
tokio = { version = "1.0", features = ["full"] }
```

### Basic Usage

```rust
use realtime_core::{Timer, RealtimeExecutor};
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a timer for 2 Hz (2 tokens per second)
    let mut timer = Timer::new(2.0)?;

    // Spawn a real-time task
    let executor = RealtimeExecutor::new()?;

    executor.spawn_realtime(async move {
        loop {
            timer.wait_for_tick().await?;
            // Process token at precise interval
            println!("Tick: {:?}", std::time::Instant::now());
        }
        #[allow(unreachable_code)]
        Ok::<_, realtime_core::TimerError>(())
    }).await?;

    Ok(())
}
```

## Timeless Principle

```rust
// This is physics: time intervals are measured in nanoseconds
// The relationship between rate and interval is mathematical truth
let interval_ns = (1_000_000_000.0 / target_rate_hz) as u64;
```

This mathematical relationship never changes. Whether you're using PREEMPT_RT, io_uring, or a simple timerfd, the physics remains: rate (Hz) and interval (ns) are inversely proportional.

## Performance Characteristics

On a properly configured PREEMPT_RT system (Linux 6.12+):

| Metric | Target | Typical |
|--------|--------|---------|
| **P50 Latency** | <500µs | ~300µs |
| **P95 Latency** | <1ms | ~700µs |
| **P99 Latency** | <2ms | ~1.2ms |
| **Worst-case Jitter** | <2ms | ~1.5ms |
| **CPU Usage** | <10% | ~5% |

On standard Linux (no PREEMPT_RT):
- **P99 Latency**: 5-10ms (higher jitter due to scheduling)
- **CPU Usage**: 15-20% (more context switches)

## System Requirements

### Minimum Requirements
- Linux 5.1+ (for io_uring support)
- Rust 1.75+
- CAP_SYS_NICE capability (for SCHED_DEADLINE)

### For <2ms Jitter Guarantee
- **Linux 6.12+** with PREEMPT_RT enabled
- **Isolated CPU cores** (kernel parameter: `isolcpus=1`)
- **IRQ affinity** configured to avoid RT cores
- **SCHED_DEADLINE** support (kernel 3.14+)

See [System Configuration](docs/USER_GUIDE.md#system-configuration) for detailed setup.

## Core Abstractions

### Timer
Hardware-level timing precision with nanosecond accuracy:
```rust
let mut timer = Timer::new(2.0)?; // 2 Hz
timer.wait_for_tick().await?; // Await precise interval
```

### Scheduler
Real-time task scheduling with CPU isolation:
```rust
let scheduler = Scheduler::new()?;
scheduler.set_cpu_affinity([1])?; // Isolate to CPU 1
scheduler.set_deadline(1_000_000, 500_000, 500_000_000)?; // runtime, deadline, period (ns)
```

### RealtimeExecutor
Async executor for real-time tasks:
```rust
let executor = RealtimeExecutor::with_config(ExecutorConfig {
    cpu_affinity: Some(vec![1]),
    scheduling_policy: SchedulingPolicy::Deadline,
    ..Default::default()
})?;
```

## Integration with Equilibrium Tokens

realtime-core is the timing foundation for the equilibrium-tokens rate control system:

```rust
use equilibrium_tokens::RateEquilibrium;
use realtime_core::Timer;

// Rate equilibrium surface with <2ms jitter
let rate_surface = RateEquilibrium::new(
    2.0,  // 2 tokens/second
    0.5,  // min 0.5 Hz
    5.0,  // max 5 Hz
)?;

// Realtime-core ensures precise token emission
let mut timer = Timer::new(2.0)?;
loop {
    timer.wait_for_tick().await?;
    rate_surface.emit_token()?;
}
```

## Use Cases

- **Audio Processing**: Sub-millisecond timing for VAD, noise suppression, ASR
- **Rate Limiting**: Precise token emission for conversation control
- **Robotics**: Hard real-time constraints for motor control
- **High-Frequency Trading**: Deterministic order execution
- **Cross-Language IPC**: Zero-copy message passing via io_uring

## Documentation

- [Architecture](docs/ARCHITECTURE.md) - System design, timeless principles, performance
- [User Guide](docs/USER_GUIDE.md) - Installation, usage, troubleshooting
- [Developer Guide](docs/DEVELOPER_GUIDE.md) - Contributing, testing, benchmarking

## Research Foundation

This library is based on comprehensive research into real-time processing systems:

- **Linux PREEMPT_RT** (6.12+): Sub-100µs worst-case latency, 8.7× jitter reduction
- **io_uring**: 30-50% jitter reduction vs. timerfd
- **CUDA Graphs**: Constant-time GPU kernel launch
- **SCHED_DEADLINE**: Hard real-time scheduling

See the [research findings](https://github.com/equilibrium-tokens/realtime-core/blob/main/docs/RESEARCH.md) for details.

## License

MIT License - see [LICENSE](LICENSE) for details.

## Acknowledgments

Built on the shoulders of giants:
- Linux PREEMPT_RT developers (20-year milestone achievement)
- Tokio async runtime team
- io_uring and kernel contributors
- Real-time Linux community

---

**The grammar is eternal.**
