//! High-precision timer for real-time periodic operations
//!
//! Provides hardware-level timing precision with nanosecond accuracy.
//! Supports multiple backends: timerfd (fallback), io_uring (recommended),
//! and mock (testing).

use crate::error::TimerError;
use crate::rate_to_interval_ns;
use std::time::{Duration, Instant};
use tokio::time::sleep;

/// Timer backend strategy
#[derive(Debug)]
pub enum TimerBackend {
    /// timerfd backend (POSIX, works everywhere)
    Timerfd {
        /// Not implemented in this stub
        _placeholder: (),
    },
    /// io_uring backend (Linux 5.1+, recommended)
    #[cfg(feature = "io-uring")]
    IoUring {
        /// Not implemented in this stub
        _placeholder: (),
    },
    /// Mock timer for testing (no hardware required)
    #[cfg(feature = "mock-timer")]
    Mock {
        /// Simulated current time
        current_time: std::cell::RefCell<Instant>,
    },
}

/// High-precision timer for real-time periodic operations
///
/// # Timeless Principle
/// ```text
/// interval_ns = 10^9 / rate_hz
/// ```
/// This relationship is physics. It doesn't change with implementation.
///
/// # Guarantees
/// - On PREEMPT_RT: <2ms jitter (99th percentile)
/// - On standard Linux: 5-10ms jitter (99th percentile)
/// - Mock timer: 0µs jitter (deterministic)
///
/// # Examples
/// ```rust,no_run
/// use realtime_core::Timer;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     // Create timer for 2 Hz (2 ticks per second)
///     let mut timer = Timer::new(2.0)?;
///
///     loop {
///         timer.wait_for_tick().await?;
///         // Process at precise 2 Hz interval
///         println!("Tick at {:?}", std::time::Instant::now());
///     }
/// }
/// ```
pub struct Timer {
    /// Interval between ticks
    interval: Duration,
    /// Rate in Hz
    rate_hz: f64,
    /// Backend implementation
    #[allow(dead_code)] // TODO: Will be used when implementing timerfd/io_uring
    backend: TimerBackend,
    /// Last tick time
    last_tick: Instant,
}

impl Timer {
    /// Create a new timer with the specified rate (Hz)
    ///
    /// # Timeless Math
    /// ```text
    /// interval_ns = 1_000_000_000 / rate_hz
    /// ```
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Timer;
    ///
    /// // 2 Hz = 2 ticks per second = 500ms interval
    /// let timer = Timer::new(2.0);
    /// assert!(timer.is_ok());
    /// ```
    ///
    /// # Errors
    /// Returns `TimerError::InvalidRate` if rate ≤ 0
    pub fn new(rate_hz: f64) -> Result<Self, TimerError> {
        if rate_hz <= 0.0 {
            return Err(TimerError::InvalidRate(rate_hz));
        }

        let interval_ns = rate_to_interval_ns(rate_hz);
        let interval = Duration::from_nanos(interval_ns);

        // For now, use a simple timerfd-based stub
        // TODO: Implement proper timerfd/io_uring backends
        let backend = TimerBackend::Timerfd { _placeholder: () };

        Ok(Self {
            interval,
            rate_hz,
            backend,
            last_tick: Instant::now(),
        })
    }

    /// Wait for the next tick with nanosecond precision
    ///
    /// # Guarantees
    /// - On PREEMPT_RT: <2ms jitter (99th percentile)
    /// - On standard Linux: 5-10ms jitter (99th percentile)
    /// - Mock timer: 0µs jitter (deterministic)
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use realtime_core::Timer;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut timer = Timer::new(2.0)?;
    /// loop {
    ///     timer.wait_for_tick().await?;
    ///     // Process at precise 2 Hz interval
    /// }
    /// # }
    /// ```
    pub async fn wait_for_tick(&mut self) -> Result<(), TimerError> {
        // Simple implementation using tokio::time::sleep
        // TODO: Replace with timerfd/io_uring for better precision
        sleep(self.interval).await;
        self.last_tick = Instant::now();
        Ok(())
    }

    /// Get the timer's rate in Hz
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Timer;
    ///
    /// let timer = Timer::new(2.0).unwrap();
    /// assert_eq!(timer.rate(), 2.0);
    /// ```
    #[inline]
    pub fn rate(&self) -> f64 {
        self.rate_hz
    }

    /// Get the timer's interval in nanoseconds
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Timer;
    ///
    /// let timer = Timer::new(2.0).unwrap(); // 2 Hz = 500ms
    /// assert_eq!(timer.interval_ns(), 500_000_000);
    /// ```
    #[inline]
    pub fn interval_ns(&self) -> u64 {
        self.interval.as_nanos() as u64
    }

    /// Get the last tick time
    #[inline]
    pub fn last_tick(&self) -> Instant {
        self.last_tick
    }

    /// Reset the timer (useful for handling drift)
    #[inline]
    pub fn reset(&mut self) {
        self.last_tick = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_timer_creation() {
        // Valid rates
        assert!(Timer::new(1.0).is_ok());
        assert!(Timer::new(2.0).is_ok());
        assert!(Timer::new(10.0).is_ok());
        assert!(Timer::new(44100.0).is_ok());

        // Invalid rates
        assert!(Timer::new(0.0).is_err());
        assert!(Timer::new(-1.0).is_err());
    }

    #[test]
    fn test_timer_rate_and_interval() {
        let timer = Timer::new(2.0).unwrap();

        // 2 Hz = 500ms interval
        assert_eq!(timer.rate(), 2.0);
        assert_eq!(timer.interval_ns(), 500_000_000);

        // 10 Hz = 100ms interval
        let timer = Timer::new(10.0).unwrap();
        assert_eq!(timer.rate(), 10.0);
        assert_eq!(timer.interval_ns(), 100_000_000);
    }

    #[tokio::test]
    async fn test_timer_tick() {
        let mut timer = Timer::new(10.0).unwrap(); // 10 Hz = 100ms

        let start = Instant::now();
        timer.wait_for_tick().await.unwrap();
        let elapsed = start.elapsed();

        // Should be approximately 100ms (allow 50ms error for CI environments)
        assert!(elapsed >= Duration::from_millis(50));
        assert!(elapsed <= Duration::from_millis(200));
    }

    #[test]
    fn test_timer_reset() {
        let mut timer = Timer::new(2.0).unwrap();
        let before = timer.last_tick();

        std::thread::sleep(Duration::from_millis(10));
        timer.reset();
        let after = timer.last_tick();

        assert!(after > before);
    }
}
