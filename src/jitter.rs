//! Jitter measurement and statistics
//!
//! Jitter is the deviation from expected timing. This module provides
//! tools to measure and analyze timing jitter with percentiles.

use std::time::Instant;

/// Single jitter measurement
///
/// # Timeless Principle
/// Jitter = |actual_interval - expected_interval|
///
/// This is physics. Positive values mean late, negative means early.
#[derive(Debug, Clone, Copy)]
pub struct JitterMeasurement {
    /// Expected interval in nanoseconds
    pub expected_interval_ns: u64,
    /// Actual interval in nanoseconds
    pub actual_interval_ns: u64,
    /// Jitter in nanoseconds (positive = late, negative = early)
    pub jitter_ns: i64,
    /// When this measurement was taken
    pub timestamp: Instant,
}

impl JitterMeasurement {
    /// Create a new jitter measurement
    ///
    /// # Examples
    /// ```
    /// use realtime_core::JitterMeasurement;
    ///
    /// // Expected: 500ms, Actual: 502ms → Jitter: +2ms (late)
    /// let measurement = JitterMeasurement::new(500_000_000, 502_000_000);
    /// assert_eq!(measurement.jitter_ns, 2_000_000);
    /// ```
    #[inline]
    pub fn new(expected_ns: u64, actual_ns: u64) -> Self {
        let jitter_ns = actual_ns as i64 - expected_ns as i64;
        Self {
            expected_interval_ns: expected_ns,
            actual_interval_ns: actual_ns,
            jitter_ns,
            timestamp: Instant::now(),
        }
    }

    /// Check if jitter is within tolerance
    ///
    /// # Examples
    /// ```
    /// use realtime_core::JitterMeasurement;
    ///
    /// let measurement = JitterMeasurement::new(500_000_000, 502_000_000);
    /// assert!(measurement.is_within_tolerance(3_000_000)); // 3ms tolerance
    /// assert!(!measurement.is_within_tolerance(1_000_000)); // 1ms tolerance
    /// ```
    #[inline]
    pub fn is_within_tolerance(&self, tolerance_ns: u64) -> bool {
        self.jitter_ns.unsigned_abs() <= tolerance_ns
    }

    /// Get absolute jitter (always positive)
    #[inline]
    pub fn abs_jitter_ns(&self) -> u64 {
        self.jitter_ns.unsigned_abs()
    }
}

/// Jitter statistics with percentiles
///
/// Provides P50, P95, P99, P99.9, and max jitter values.
#[derive(Debug, Clone, Default)]
pub struct JitterStats {
    /// 50th percentile (median)
    pub p50_ns: u64,
    /// 95th percentile
    pub p95_ns: u64,
    /// 99th percentile
    pub p99_ns: u64,
    /// 99.9th percentile
    pub p999_ns: u64,
    /// Maximum jitter observed
    pub max_ns: u64,
    /// Number of measurements
    pub count: usize,
}

impl JitterStats {
    /// Calculate statistics from a collection of measurements
    ///
    /// # Examples
    /// ```
    /// use realtime_core::{JitterMeasurement, JitterStats};
    ///
    /// let measurements = vec![
    ///     JitterMeasurement::new(500_000_000, 500_001_000), // +1ms
    ///     JitterMeasurement::new(500_000_000, 500_002_000), // +2ms
    ///     JitterMeasurement::new(500_000_000, 499_999_000), // -1ms
    /// ];
    ///
    /// let stats = JitterStats::from_measurements(&measurements);
    /// assert_eq!(stats.count, 3);
    /// ```
    pub fn from_measurements(measurements: &[JitterMeasurement]) -> Self {
        if measurements.is_empty() {
            return Self::default();
        }

        let mut jitter_values: Vec<u64> = measurements
            .iter()
            .map(|m| m.abs_jitter_ns())
            .collect();

        jitter_values.sort_unstable();

        Self {
            p50_ns: percentile(&jitter_values, 50.0),
            p95_ns: percentile(&jitter_values, 95.0),
            p99_ns: percentile(&jitter_values, 99.0),
            p999_ns: percentile(&jitter_values, 99.9),
            max_ns: *jitter_values.last().unwrap_or(&0),
            count: measurements.len(),
        }
    }

    /// Check if P99 jitter is within tolerance
    ///
    /// # Examples
    /// ```
    /// use realtime_core::{JitterMeasurement, JitterStats};
    ///
    /// let measurements = vec![
    ///     JitterMeasurement::new(500_000_000, 501_000_000), // +1ms
    ///     JitterMeasurement::new(500_000_000, 500_500_000), // +0.5ms
    /// ];
    ///
    /// let stats = JitterStats::from_measurements(&measurements);
    /// assert!(stats.p99_within_tolerance(2_000_000)); // 2ms
    /// ```
    #[inline]
    pub fn p99_within_tolerance(&self, tolerance_ns: u64) -> bool {
        self.p99_ns <= tolerance_ns
    }

    /// Get P99 jitter in milliseconds
    #[inline]
    pub fn p99_ms(&self) -> f64 {
        self.p99_ns as f64 / 1_000_000.0
    }

    /// Get max jitter in milliseconds
    #[inline]
    pub fn max_ms(&self) -> f64 {
        self.max_ns as f64 / 1_000_000.0
    }
}

/// Calculate percentile from sorted values
///
/// # Timeless Math
/// Linear interpolation between nearest ranks:
/// ```text
/// index = (percentile / 100) * (count - 1)
/// ```
fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }

    if sorted.len() == 1 {
        return sorted[0];
    }

    let index = (p / 100.0) * (sorted.len() - 1) as f64;
    let lower = index.floor() as usize;
    let upper = index.ceil() as usize;

    if lower == upper {
        return sorted[lower];
    }

    // Linear interpolation
    let weight = index - lower as f64;
    let lower_val = sorted[lower] as f64;
    let upper_val = sorted[upper] as f64;

    (lower_val + weight * (upper_val - lower_val)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jitter_measurement() {
        // Expected: 500ms, Actual: 502ms → Jitter: +2ms (late)
        let measurement = JitterMeasurement::new(500_000_000, 502_000_000);
        assert_eq!(measurement.expected_interval_ns, 500_000_000);
        assert_eq!(measurement.actual_interval_ns, 502_000_000);
        assert_eq!(measurement.jitter_ns, 2_000_000);
        assert_eq!(measurement.abs_jitter_ns(), 2_000_000);

        // Expected: 500ms, Actual: 498ms → Jitter: -2ms (early)
        let measurement = JitterMeasurement::new(500_000_000, 498_000_000);
        assert_eq!(measurement.jitter_ns, -2_000_000);
        assert_eq!(measurement.abs_jitter_ns(), 2_000_000);
    }

    #[test]
    fn test_within_tolerance() {
        let measurement = JitterMeasurement::new(500_000_000, 502_000_000);

        // +2ms jitter
        assert!(measurement.is_within_tolerance(3_000_000)); // 3ms tolerance
        assert!(measurement.is_within_tolerance(2_000_000)); // 2ms tolerance
        assert!(!measurement.is_within_tolerance(1_000_000)); // 1ms tolerance
    }

    #[test]
    fn test_jitter_stats_empty() {
        let measurements: Vec<JitterMeasurement> = vec![];
        let stats = JitterStats::from_measurements(&measurements);

        assert_eq!(stats.count, 0);
        assert_eq!(stats.p50_ns, 0);
        assert_eq!(stats.p95_ns, 0);
        assert_eq!(stats.p99_ns, 0);
    }

    #[test]
    fn test_jitter_stats_single() {
        let measurements = vec![JitterMeasurement::new(500_000_000, 502_000_000)];
        let stats = JitterStats::from_measurements(&measurements);

        assert_eq!(stats.count, 1);
        assert_eq!(stats.p50_ns, 2_000_000);
        assert_eq!(stats.p95_ns, 2_000_000);
        assert_eq!(stats.p99_ns, 2_000_000);
        assert_eq!(stats.max_ns, 2_000_000);
    }

    #[test]
    fn test_jitter_stats_multiple() {
        let measurements = vec![
            JitterMeasurement::new(500_000_000, 500_001_000), // +1000ns
            JitterMeasurement::new(500_000_000, 500_002_000), // +2000ns
            JitterMeasurement::new(500_000_000, 500_003_000), // +3000ns
            JitterMeasurement::new(500_000_000, 500_004_000), // +4000ns
            JitterMeasurement::new(500_000_000, 500_005_000), // +5000ns
        ];

        let stats = JitterStats::from_measurements(&measurements);

        assert_eq!(stats.count, 5);
        assert_eq!(stats.p50_ns, 3000); // median of [1000,2000,3000,4000,5000]
        assert_eq!(stats.max_ns, 5000);
    }

    #[test]
    fn test_percentile() {
        let values = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

        // P50: index = 0.50 * (10-1) = 4.5
        // Interpolate between values[4]=5 and values[5]=6
        // Result: 5 + 0.5 * (6 - 5) = 5.5
        let p50 = percentile(&values, 50.0);
        assert!(p50 == 5 || p50 == 6); // Accept either due to rounding

        // P95: index = 0.95 * (10-1) = 8.55
        // Interpolate between values[8]=9 and values[9]=10
        // Result: 9 + 0.55 * (10 - 9) = 9.55
        assert_eq!(percentile(&values, 95.0), 9); // P95 rounds to 9

        assert_eq!(percentile(&values, 0.0), 1); // P0 (min)
        assert_eq!(percentile(&values, 100.0), 10); // P100 (max)
    }

    #[test]
    fn test_p99_within_tolerance() {
        let measurements = vec![
            JitterMeasurement::new(500_000_000, 501_000_000), // +1ms
            JitterMeasurement::new(500_000_000, 500_500_000), // +0.5ms
            JitterMeasurement::new(500_000_000, 501_500_000), // +1.5ms
        ];

        let stats = JitterStats::from_measurements(&measurements);

        assert!(stats.p99_within_tolerance(2_000_000)); // 2ms
        assert!(!stats.p99_within_tolerance(1_000_000)); // 1ms
    }

    #[test]
    fn test_jitter_ms_conversion() {
        let measurements = vec![JitterMeasurement::new(500_000_000, 502_000_000)]; // +2ms
        let stats = JitterStats::from_measurements(&measurements);

        assert!((stats.p99_ms() - 2.0).abs() < 0.001);
        assert!((stats.max_ms() - 2.0).abs() < 0.001);
    }
}
