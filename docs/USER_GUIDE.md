# realtime-core User Guide

**Complete guide to using deterministic timing primitives in your applications**

## Table of Contents

1. [Installation](#installation)
2. [System Configuration](#system-configuration)
3. [Basic Usage](#basic-usage)
4. [Advanced Usage](#advanced-usage)
5. [Integration Examples](#integration-examples)
6. [Performance Tuning](#performance-tuning)
7. [Troubleshooting](#troubleshooting)

---

## Installation

### Requirements

#### Minimum Requirements
- **Linux 5.1+** (for io_uring support)
- **Rust 1.75+**
- **CAP_SYS_NICE** capability (for SCHED_DEADLINE)

#### For <2ms Jitter Guarantee
- **Linux 6.12+** with PREEMPT_RT enabled
- **Isolated CPU cores**
- **IRQ affinity** configured

### Installing PREEMPT_RT (Recommended)

#### Option 1: Use PREEMPT_RT Kernel (Ubuntu/Debian)

```bash
# Install PREEMPT_RT kernel
sudo apt install linux-image-rt-amd64 linux-headers-rt-amd64

# Update GRUB to use RT kernel
sudo sed -i 's/GRUB_DEFAULT=0/GRUB_DEFAULT=1/' /etc/default/grub
sudo update-grub

# Reboot into RT kernel
sudo reboot

# Verify PREEMPT_RT is active
uname -v  # Should show "PREEMPT_RT"
```

#### Option 2: Compile Custom Kernel with PREEMPT_RT

```bash
# Download Linux 6.12+ kernel
wget https://cdn.kernel.org/pub/linux/kernel/v6.x/linux-6.12.tar.xz
tar -xf linux-6.12.tar.xz
cd linux-6.12

# Download PREEMPT_RT patch
wget https://cdn.kernel.org/pub/linux/kernel/projects/rt/6.12/patch-6.12-rt12.patch.xz
xzcat patch-6.12-rt12.patch.xz | patch -p1

# Configure kernel with PREEMPT_RT enabled
make olddefconfig
echo "CONFIG_PREEMPT_RT=y" >> .config

# Build and install
make -j$(nproc)
sudo make modules_install install
sudo update-grub

# Reboot
sudo reboot
```

### Add to Your Project

Add to `Cargo.toml`:

```toml
[dependencies]
realtime-core = "0.1"
tokio = { version = "1.0", features = ["full"] }
```

For maximum performance (PREEMPT_RT + io_uring):

```toml
[dependencies]
realtime-core = { version = "0.1", features = ["full"] }
```

### Verify Installation

```bash
# Test basic timer functionality
cargo run --example basic_timer

# Test PREEMPT_RT support
cargo run --example cpu_isolation

# Benchmark jitter
cargo bench --bench timer_jitter
```

---

## System Configuration

### CPU Isolation

Prevent other processes from running on real-time cores:

```bash
# Edit GRUB configuration
sudo nano /etc/default/grub

# Add to GRUB_CMDLINE_LINUX_DEFAULT:
# isolcpus=1,2  # Isolate CPUs 1 and 2 for real-time tasks

# Update GRUB and reboot
sudo update-grub
sudo reboot

# Verify isolation
cat /sys/devices/system/cpu/isolated
# Output: 1-2
```

### IRQ Affinity

Route interrupts away from isolated CPUs:

```bash
# Move all IRQs to CPU 0
for irq in $(ls /proc/irq/ | grep -E '[0-9]+'); do
    echo 1 > /proc/irq/$irq/smp_affinity_list  # CPU 0 only
done

# Make this persistent across reboots
echo "IRQ Affinity Configuration" | sudo tee -a /etc/rc.local
for irq in $(ls /proc/irq/ | grep -E '[0-9]+'); do
    echo "echo 1 > /proc/irq/$irq/smp_affinity_list" | sudo tee -a /etc/rc.local
done
sudo chmod +x /etc/rc.local
```

### System Tuning with `tuned`

```bash
# Install tuned
sudo apt install tuned

# Create real-time profile
sudo mkdir -p /etc/tuned/realtime
sudo tee /etc/tuned/realtime/tuned.conf <<EOF
[main]
summary=Real-time optimization for low-latency applications

[sysctl]
kernel.sched_min_granularity_ns=1000000
kernel.sched_wakeup_granularity_ns=1500000
kernel.sched_latency_ns=20000000
vm.swappiness=10

[irqbalance]
disabled=1

[video]
powersave=0

[cpu]
governor=performance
energy_perf_bias=performance
EOF

# Enable profile
sudo tuned-adm profile realtime
```

### Disable Transparent Huge Pages

Reduces latency spikes from defragmentation:

```bash
# Disable THP
echo never | sudo tee /sys/kernel/mm/transparent_hugepage/enabled

# Make persistent
echo "echo never > /sys/kernel/mm/transparent_hugepage/enabled" | \
    sudo tee -a /etc/rc.local
```

---

## Basic Usage

### Creating a Timer

```rust
use realtime_core::Timer;
use std::error::Error;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Create a 2 Hz timer (2 ticks per second)
    let mut timer = Timer::new(2.0)?;

    loop {
        // Wait for next tick with <2ms jitter
        timer.wait_for_tick().await?;

        // Your code here runs at precise 2 Hz
        println!("Tick at {:?}", std::time::Instant::now());
    }
}
```

### Rate Control for Equilibrium Tokens

```rust
use equilibrium_tokens::RateEquilibrium;
use realtime_core::Timer;
use tokio::time::{timeout, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create rate equilibrium surface
    let rate_surface = RateEquilibrium::new(2.0, 0.5, 5.0)?;

    // Create real-time timer
    let mut timer = Timer::new(2.0)?;

    let mut ticks = 0u64;

    loop {
        // Wait for precise tick
        timer.wait_for_tick().await?;

        // Emit token at exact interval
        rate_surface.emit_token()?;
        ticks += 1;

        if ticks % 10 == 0 {
            println!("Emitted {} tokens at 2 Hz", ticks);
        }

        // Optional: Add timeout to detect missed ticks
        let emission = timeout(Duration::from_millis(100), async {
            // Token processing logic
            Ok::<(), Box<dyn std::error::Error>>(())
        }).await??;
    }
}
```

### Monitoring Jitter

```rust
use realtime_core::{Timer, JitterMeasurement, JitterStats};
use std::time::Instant;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut timer = Timer::new(10.0)?;  // 10 Hz
    let mut measurements = Vec::new();
    let mut last_tick = Instant::now();

    loop {
        timer.wait_for_tick().await?;
        let now = Instant::now();

        let expected_interval = timer.interval_ns();
        let actual_interval = now.duration_since(last_tick).as_nanos() as u64;

        measurements.push(JitterMeasurement::new(expected_interval, actual_interval));

        // Calculate statistics every 100 ticks
        if measurements.len() >= 100 {
            let stats = JitterStats::from_measurements(&measurements);

            println!("Jitter Stats:");
            println!("  P50: {} µs", stats.p50_ns / 1000);
            println!("  P95: {} µs", stats.p95_ns / 1000);
            println!("  P99: {} µs", stats.p99_ns / 1000);
            println!("  Max: {} µs", stats.max_ns / 1000);

            // Alert if jitter exceeds 2ms target
            if stats.p99_ns > 2_000_000 {
                eprintln!("WARNING: P99 jitter exceeds 2ms target!");
            }

            measurements.clear();
        }

        last_tick = now;
    }
}
```

---

## Advanced Usage

### CPU Isolation for Real-Time Tasks

```rust
use realtime_core::{RealtimeExecutor, ExecutorConfig, SchedulingPolicy};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create executor with CPU isolation
    let executor = RealtimeExecutor::with_config(ExecutorConfig {
        cpu_affinity: Some(vec![1]),  // Isolate to CPU 1
        scheduling_policy: SchedulingPolicy::Fifo(50),  // Priority 50
        worker_threads: Some(1),
        enable_parking: false,  // Disable parking for low latency
        ..Default::default()
    })?;

    // Spawn real-time task
    executor.spawn_realtime(async move {
        let mut timer = Timer::new(100.0)?;  // 100 Hz

        loop {
            timer.wait_for_tick().await?;
            // Runs on isolated CPU 1 with FIFO scheduling
        }
        #[allow(unreachable_code)]
        Ok::<_, Box<dyn std::error::Error>>(())
    }).await?;

    Ok(())
}
```

### SCHED_DEADLINE for Hard Real-Time Guarantees

```rust
use realtime_core::{Scheduler, SchedulingPolicy};

fn configure_deadline_scheduler() -> Result<(), Box<dyn std::error::Error>> {
    let mut scheduler = Scheduler::new()?;

    // Set SCHED_DEADLINE parameters
    // Example: 2 Hz rate control (500ms period)
    scheduler.set_deadline(
        1_000_000,      // runtime: 1ms CPU time per period
        500_000,        // deadline: complete within 500µs
        500_000_000,    // period: 500ms (2 Hz)
    )?;

    // Apply to current thread
    scheduler.apply_to_current_thread()?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    configure_deadline_scheduler()?;

    let mut timer = Timer::new(2.0)?;
    loop {
        timer.wait_for_tick().await?;
        // Guaranteed to complete within 500µs deadline
    }
}
```

### Using io_uring for Lowest Jitter

```rust
use realtime_core::Timer;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Enable io_uring feature in Cargo.toml:
    // realtime-core = { version = "0.1", features = ["io-uring"] }

    let mut timer = Timer::new(10.0)?;  // 10 Hz

    // Timer automatically uses io_uring backend if feature enabled
    loop {
        timer.wait_for_tick().await?;
        // Sub-microsecond I/O latency with io_uring
    }
}
```

### Cross-Language Integration with Python

```rust
use realtime_core::Timer;
use pyo3::prelude::*;

#[pyclass]
pub struct PythonRateTimer {
    timer: Timer,
}

#[pymethods]
impl PythonRateTimer {
    #[new]
    fn new(rate_hz: f64) -> PyResult<Self> {
        Ok(Self {
            timer: Timer::new(rate_hz)?,
        })
    }

    fn wait_for_tick<'py>(&mut self, py: Python<'py>) -> PyResult<&'py PyAny> {
        // Release GIL during timer wait
        py.allow_threads(|| {
            // Wait for tick (this is where Rust speed shines)
            futures::executor::block_on(self.timer.wait_for_tick())
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
        })
    }

    fn rate(&self) -> f64 {
        self.timer.rate()
    }
}

#[pymodule]
fn realtime_core_py(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PythonRateTimer>()?;
    Ok(())
}
```

**Usage from Python:**

```python
import realtime_core_py

# Create 2 Hz timer
timer = realtime_core_py.PythonRateTimer(2.0)

while True:
    timer.wait_for_tick()  # Rust speed, Python convenience
    # Process token with <2ms jitter
```

---

## Integration Examples

### Example 1: Audio Pipeline with VAD

```rust
use realtime_core::{Timer, RealtimeExecutor};
use audio_pipeline::{VAD, AudioProcessor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = RealtimeExecutor::new()?;
    let mut timer = Timer::new(30.0)?;  // 30 Hz (30ms audio frames)

    let mut vad = VAD::new();
    let mut processor = AudioProcessor::new()?;

    executor.spawn_realtime(async move {
        loop {
            timer.wait_for_tick().await?;

            // Process 30ms audio frame with <2ms jitter
            let frame = processor.read_frame()?;
            let is_speech = vad.detect(&frame)?;

            if is_speech {
                // Trigger interruption handling
            }
        }
        #[allow(unreachable_code)]
        Ok::<_, Box<dyn std::error::Error>>(())
    }).await?;

    Ok(())
}
```

### Example 2: GPU Synchronization

```rust
use realtime_core::{Timer, RealtimeExecutor};
use gpu_accelerator::{CudaDevice, CudaGraph};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = RealtimeExecutor::with_config(ExecutorConfig {
        cpu_affinity: Some(vec![1]),
        ..Default::default()
    })?;

    let device = CudaDevice::new(0)?;
    let mut timer = Timer::new(60.0)?;  // 60 Hz for GPU sync

    executor.spawn_realtime(async move {
        // Pre-compile CUDA graph for sentiment inference
        let graph = device.create_sentiment_graph()?;

        loop {
            timer.wait_for_tick().await?;

            // Launch CUDA Graph (constant-time, no kernel launch overhead)
            graph.launch()?;

            // GPU operation completes in <5ms
        }
        #[allow(unreachable_code)]
        Ok::<_, Box<dyn std::error::Error>>(())
    }).await?;

    Ok(())
}
```

### Example 3: Multi-Rate Coordination

```rust
use realtime_core::{Timer, RealtimeExecutor};
use tokio::select;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = RealtimeExecutor::new()?;

    // Rate control: 2 Hz
    let mut rate_timer = Timer::new(2.0)?;

    // VAD: 30 Hz (30ms frames)
    let mut vad_timer = Timer::new(30.0)?;

    // Sentiment: 10 Hz (100ms updates)
    let mut sentiment_timer = Timer::new(10.0)?;

    loop {
        tokio::select! {
            _ = rate_timer.wait_for_tick() => {
                // Emit token at 2 Hz
                println!("Rate tick");
            }
            _ = vad_timer.wait_for_tick() => {
                // Process VAD at 30 Hz
                println!("VAD tick");
            }
            _ = sentiment_timer.wait_for_tick() => {
                // Update sentiment at 10 Hz
                println!("Sentiment tick");
            }
        }
    }
}
```

---

## Performance Tuning

### Measuring Your Jitter

```bash
# Use cyclictest to measure system jitter
sudo apt install rt-tests

# Test for 60 seconds on CPU 1
sudo cyclictest -p 80 -t1 -n -i 10000 -l 6000 -a 1

# Expected output on PREEMPT_RT:
# P99: <100µs
# Max: <500µs
```

### Optimizing Timer Frequency

**Higher Frequency (>100 Hz)**:
- Use io_uring backend
- Disable thread parking
- SCHED_FIFO with priority 80+

```rust
let executor = RealtimeExecutor::with_config(ExecutorConfig {
    cpu_affinity: Some(vec![1]),
    scheduling_policy: SchedulingPolicy::Fifo(80),
    enable_parking: false,  // Critical for high frequency
    ..Default::default()
})?;
```

**Lower Frequency (<10 Hz)**:
- timerfd backend sufficient
- Can enable thread parking
- Lower priority acceptable

### Reducing Context Switches

```bash
# Pin process to specific CPU
taskset -c 1 cargo run --example rate_control

# Or use taskset from within Rust
std::process::Command::new("taskset")
    .args(&["-p", "-c", "1", &std::process::id().to_string()])
    .output()?;
```

### Memory Locking

Prevent page faults in timing-critical sections:

```rust
use libc::{mlockall, MCL_CURRENT, MCL_FUTURE};

fn lock_memory() -> Result<(), std::io::Error> {
    unsafe {
        let ret = mlockall(MCL_CURRENT | MCL_FUTURE);
        if ret != 0 {
            return Err(std::io::Error::last_os_error());
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Lock all current and future memory pages
    lock_memory()?;

    let mut timer = Timer::new(10.0)?;
    loop {
        timer.wait_for_tick().await?;
        // No page faults will occur here
    }
}
```

---

## Troubleshooting

### Problem: Jitter Exceeds 2ms

**Symptoms**: P99 jitter > 2ms in metrics

**Diagnosis**:
```bash
# Check if PREEMPT_RT is enabled
uname -v | grep -i preempt

# Check CPU isolation
cat /sys/devices/system/cpu/isolated

# Check IRQ affinity
cat /proc/irq/*/smp_affinity_list

# Measure system jitter
sudo cyclictest -p 80 -t1 -n -i 10000 -l 6000
```

**Solutions**:
1. Enable PREEMPT_RT kernel
2. Isolate CPU cores (`isolcpus=1` in GRUB)
3. Move IRQs away from isolated CPUs
4. Use SCHED_DEADLINE or high-priority SCHED_FIFO
5. Disable power management (`cpupower frequency-set -g performance`)

### Problem: Timer Creation Fails

**Symptoms**: `Timer::new()` returns error

**Diagnosis**:
```bash
# Check kernel version
uname -r  # Should be 5.1+ for io_uring, 6.12+ for PREEMPT_RT

# Check capabilities
capsh --print | grep CAP_SYS_NICE

# Check if io_uring is available
ls /proc/sys/fs/io_uring
```

**Solutions**:
1. Update kernel to 6.12+
2. Grant CAP_SYS_NICE: `sudo setcap cap_sys_nice+ep your_binary`
3. Use timerfd backend if io_uring unavailable: `features = ["timerfd"]`

### Problem: "Permission Denied" Setting Scheduling Policy

**Symptoms**: `Scheduler::apply_to_current_thread()` fails

**Diagnosis**:
```bash
# Check current user
whoami

# Check capabilities
capsh --print
```

**Solutions**:
1. Run with `sudo` (not recommended for production)
2. Grant CAP_SYS_NICE capability:
   ```bash
   sudo setcap cap_sys_nice+ep /path/to/your/binary
   ```
3. Add user to `realtime` group (distro-specific)

### Problem: Inconsistent Timing Across Runs

**Symptoms**: Jitter varies widely between executions

**Diagnosis**:
```bash
# Check CPU frequency scaling
cpupower frequency-info

# Check for thermal throttling
watch -n 1 'cat /sys/class/thermal/thermal_zone*/temp'

# Check for background processes
htop
```

**Solutions**:
1. Disable CPU frequency scaling: `cpupower frequency-set -g performance`
2. Cool down system (reduce thermal throttling)
3. Stop non-essential background services
4. Use CPU isolation more aggressively

### Problem: Timer Drift Over Time

**Symptoms**: Timer slowly drifts from expected interval

**Diagnosis**:
```rust
// Measure drift over time
let mut timer = Timer::new(1.0)?;  // 1 Hz
let start = Instant::now();

for i in 0..1000 {
    timer.wait_for_tick().await?;
    let elapsed = start.elapsed().as_secs_f64();
    let expected = i as f64 / 1.0;
    println!("Drift: {} ms", (elapsed - expected) * 1000.0);
}
```

**Solutions**:
1. Use `CLOCK_MONOTONIC` (default, immune to NTP adjustments)
2. Avoid `CLOCK_REALTIME` for timing-critical code
3. Recalculate interval periodically if needed:
   ```rust
   let mut tick_count = 0u64;
   let start = Instant::now();

   loop {
       timer.wait_for_tick().await?;
       tick_count += 1;

       // Recalculate every 1000 ticks to compensate for drift
       if tick_count % 1000 == 0 {
           let elapsed = start.elapsed();
           let actual_rate = tick_count as f64 / elapsed.as_secs_f64();
           timer = Timer::new(actual_rate)?;
       }
   }
   ```

### Getting Help

If issues persist:

1. **Check logs**: Enable debug logging
   ```rust
   tracing_subscriber::fmt()
       .with_max_level(tracing::Level::DEBUG)
       .init();
   ```

2. **Run diagnostics**:
   ```bash
   # System info
   uname -a
   cat /proc/cmdline

   # realtime-core version
   cargo tree | grep realtime-core

   # Jitter test
   cargo bench --bench timer_jitter
   ```

3. **File issue** with:
   - Kernel version (`uname -r`)
   - PREEMPT_RT status
   - Jitter measurements
   - Minimal reproducible example

---

## Best Practices

1. **Always use CLOCK_MONOTONIC** for timing (default)
2. **Isolate CPU cores** for real-time tasks
3. **Route IRQs away** from isolated cores
4. **Use SCHED_DEADLINE** for hard real-time guarantees
5. **Monitor jitter** in production with metrics
6. **Test with cyclictest** before deploying
7. **Lock memory** to prevent page faults
8. **Disable power management** on real-time cores
9. **Use io_uring** for lowest jitter (Linux 5.1+)
10. **Have fallback** for systems without PREEMPT_RT

---

**Next**: See [Developer Guide](DEVELOPER_GUIDE.md) for contributing, testing, and benchmarking.
