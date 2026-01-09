//! realtime-core: Deterministic timing primitives for real-time systems
//!
//! This library provides hardware-level timing precision for Rust applications
//! requiring sub-millisecond jitter guarantees. Built on Linux PREEMPT_RT,
//! io_uring, and SCHED_DEADLINE, it delivers deterministic timing for audio
//! processing, high-frequency trading, and robotics.
//!
//! # Timeless Principle
//!
//! ```text
//! interval_ns = 10^9 / rate_hz
//! ```
//!
//! This relationship is physics. It doesn't change with implementation.
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use realtime_core::{Timer, RealtimeExecutor};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create a timer for 2 Hz (2 tokens per second)
//!     let mut timer = Timer::new(2.0)?;
//!
//!     // Spawn a real-time task
//!     let executor = RealtimeExecutor::new()?;
//!
//!     executor.spawn_realtime(async move {
//!         loop {
//!             timer.wait_for_tick().await?;
//!             // Process token at precise interval
//!             println!("Tick: {:?}", std::time::Instant::now());
//!         }
//!         #[allow(unreachable_code)]
//!         Ok::<_, realtime_core::TimerError>(())
//!     }).await?;
//!
//!     Ok(())
//! }
//! ```

#![warn(missing_docs)]
#![warn(clippy::all)]

// Public modules
pub mod error;
pub mod executor;
pub mod jitter;
pub mod scheduler;
pub mod timer;

// Re-export key types
pub use error::{ExecutorError, SchedulerError, TimerError};
pub use executor::{ExecutorConfig, ExecutorMetrics, RealtimeExecutor};
pub use jitter::{JitterMeasurement, JitterStats};
pub use scheduler::{DeadlineParams, SchedulingPolicy};
pub use timer::Timer;

/// Timeless constant: nanoseconds per second
pub const NANOS_PER_SECOND: u64 = 1_000_000_000;

/// Convert rate (Hz) to interval (nanoseconds)
///
/// # Timeless Math
/// ```text
/// interval_ns = 10^9 / rate_hz
/// ```
///
/// This is physics, not implementation. It will never change.
///
/// # Examples
/// ```
/// use realtime_core::rate_to_interval_ns;
///
/// // 1 Hz = 1 second = 1,000,000,000 ns
/// assert_eq!(rate_to_interval_ns(1.0), 1_000_000_000);
///
/// // 2 Hz = 0.5 seconds = 500,000,000 ns
/// assert_eq!(rate_to_interval_ns(2.0), 500_000_000);
///
/// // 10 Hz = 0.1 seconds = 100,000,000 ns
/// assert_eq!(rate_to_interval_ns(10.0), 100_000_000);
/// ```
#[inline]
pub fn rate_to_interval_ns(rate_hz: f64) -> u64 {
    assert!(rate_hz > 0.0, "Rate must be positive");
    (NANOS_PER_SECOND as f64 / rate_hz) as u64
}

/// Convert interval (nanoseconds) to rate (Hz)
///
/// # Timeless Math
/// ```text
/// rate_hz = 10^9 / interval_ns
/// ```
///
/// This is physics, not implementation. It will never change.
///
/// # Examples
/// ```
/// use realtime_core::interval_ns_to_rate;
///
/// // 1,000,000,000 ns = 1 second = 1 Hz
/// assert_eq!(interval_ns_to_rate(1_000_000_000), 1.0);
///
/// // 500,000,000 ns = 0.5 seconds = 2 Hz
/// assert_eq!(interval_ns_to_rate(500_000_000), 2.0);
///
/// // 100,000,000 ns = 0.1 seconds = 10 Hz
/// assert_eq!(interval_ns_to_rate(100_000_000), 10.0);
/// ```
#[inline]
pub fn interval_ns_to_rate(interval_ns: u64) -> f64 {
    assert!(interval_ns > 0, "Interval must be positive");
    NANOS_PER_SECOND as f64 / interval_ns as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_interval_inverse() {
        // 1 Hz = 1 second = 1,000,000,000 ns
        assert_eq!(rate_to_interval_ns(1.0), 1_000_000_000);
        assert_eq!(interval_ns_to_rate(1_000_000_000), 1.0);

        // 2 Hz = 0.5 seconds = 500,000,000 ns
        assert_eq!(rate_to_interval_ns(2.0), 500_000_000);
        assert_eq!(interval_ns_to_rate(500_000_000), 2.0);

        // 10 Hz = 0.1 seconds = 100,000,000 ns
        assert_eq!(rate_to_interval_ns(10.0), 100_000_000);
        assert_eq!(interval_ns_to_rate(100_000_000), 10.0);
    }

    #[test]
    fn test_roundtrip_conversion() {
        let test_rates = [0.5, 1.0, 2.0, 10.0, 60.0, 100.0, 44100.0];

        for &original_rate in &test_rates {
            let interval = rate_to_interval_ns(original_rate);
            let recovered_rate = interval_ns_to_rate(interval);

            // Allow small floating point errors
            let error = (recovered_rate - original_rate).abs() / original_rate;
            assert!(
                error < 0.0001,
                "Roundtrip error too large: {} -> {} -> {} (error: {})",
                original_rate, interval, recovered_rate, error
            );
        }
    }

    #[test]
    #[should_panic(expected = "Rate must be positive")]
    fn test_negative_rate() {
        rate_to_interval_ns(-1.0);
    }

    #[test]
    #[should_panic(expected = "Rate must be positive")]
    fn test_zero_rate() {
        rate_to_interval_ns(0.0);
    }

    #[test]
    #[should_panic(expected = "Interval must be positive")]
    fn test_zero_interval() {
        interval_ns_to_rate(0);
    }
}
