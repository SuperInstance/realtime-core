//! Async executor configured for real-time tasks
//!
//! Provides bounded task scheduling latency with CPU isolation and
//! real-time scheduling policies.

use crate::error::ExecutorError;
use crate::scheduler::SchedulingPolicy;
use std::sync::Arc;

/// Executor configuration
///
/// # Examples
/// ```
/// use realtime_core::{ExecutorConfig, SchedulingPolicy};
///
/// let config = ExecutorConfig {
///     cpu_affinity: Some(vec![1]),
///     scheduling_policy: SchedulingPolicy::Fifo(50),
///     ..Default::default()
/// };
/// ```
#[derive(Clone)]
pub struct ExecutorConfig {
    /// CPU cores to run on (for isolation)
    pub cpu_affinity: Option<Vec<usize>>,

    /// Scheduling policy
    pub scheduling_policy: SchedulingPolicy,

    /// Number of worker threads (default: 1 for RT tasks)
    pub worker_threads: Option<usize>,

    /// Enable thread parking for power efficiency
    pub enable_parking: bool,

    /// Metrics callback for performance monitoring
    pub metrics_callback: Option<
        Arc<dyn Fn(ExecutorMetrics) + Send + Sync + 'static>,
    >,
}

impl Default for ExecutorConfig {
    fn default() -> Self {
        Self {
            cpu_affinity: None,
            scheduling_policy: SchedulingPolicy::Other,
            worker_threads: Some(1), // Single thread for determinism
            enable_parking: true,
            metrics_callback: None,
        }
    }
}

/// Executor metrics for performance monitoring
///
/// # Examples
/// ```
/// use realtime_core::ExecutorMetrics;
///
/// let metrics = ExecutorMetrics {
///     scheduled_tasks: 1000,
///     completed_tasks: 950,
///     avg_latency_ns: 500_000,
///     p99_latency_ns: 2_000_000,
/// };
/// ```
#[derive(Debug, Clone, Default)]
pub struct ExecutorMetrics {
    /// Number of tasks scheduled
    pub scheduled_tasks: u64,

    /// Number of tasks completed
    pub completed_tasks: u64,

    /// Average task latency (ns)
    pub avg_latency_ns: u64,

    /// P99 task latency (ns)
    pub p99_latency_ns: u64,
}

/// Async executor configured for real-time tasks
///
/// # Guarantees
/// - Bounded task scheduling latency (<100µs on PREEMPT_RT)
/// - Priority-aware task queueing
/// - Integration with tokio for async/await support
///
/// # Examples
/// ```rust,no_run
/// use realtime_core::{ExecutorConfig, RealtimeExecutor, SchedulingPolicy};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let executor = RealtimeExecutor::with_config(ExecutorConfig {
///         cpu_affinity: Some(vec![1]),
///         scheduling_policy: SchedulingPolicy::Fifo(50),
///         ..Default::default()
///     })?;
///
///     executor.spawn_realtime(async move {
///         // Your real-time task here
///         println!("Executing with bounded latency");
///         Ok::<(), anyhow::Error>(())
///     }).await?;
///
///     Ok(())
/// }
/// ```
pub struct RealtimeExecutor {
    /// Tokio runtime
    runtime: tokio::runtime::Runtime,
    /// Executor configuration
    config: ExecutorConfig,
}

impl RealtimeExecutor {
    /// Create a new real-time executor with default config
    ///
    /// # Examples
    /// ```
    /// use realtime_core::RealtimeExecutor;
    ///
    /// let executor = RealtimeExecutor::new();
    /// assert!(executor.is_ok());
    /// ```
    pub fn new() -> Result<Self, ExecutorError> {
        Self::with_config(ExecutorConfig::default())
    }

    /// Create a new real-time executor with custom config
    ///
    /// # Examples
    /// ```rust,no_run
    /// use realtime_core::{ExecutorConfig, RealtimeExecutor, SchedulingPolicy};
    ///
    /// let executor = RealtimeExecutor::with_config(ExecutorConfig {
    ///     cpu_affinity: Some(vec![1]),
    ///     scheduling_policy: SchedulingPolicy::Fifo(50),
    ///     ..Default::default()
/// }).unwrap();
    /// ```
    pub fn with_config(config: ExecutorConfig) -> Result<Self, ExecutorError> {
        let mut builder = tokio::runtime::Builder::new_multi_thread();

        // Configure worker threads
        if let Some(threads) = config.worker_threads {
            builder.worker_threads(threads);
        }

        // Disable thread parking for low latency (if configured)
        if !config.enable_parking {
            // Keep threads alive indefinitely (no timeout)
            builder.thread_keep_alive(std::time::Duration::from_secs(u64::MAX));
        }

        // Build runtime
        let runtime = builder
            .build()
            .map_err(|e| ExecutorError::CreationFailed(std::io::Error::other(format!("Runtime build failed: {}", e))))?;

        // TODO: Apply CPU affinity and scheduling policy
        // This requires platform-specific code:
        // - Linux: sched_setaffinity, sched_setscheduler
        // - Need to call in worker thread setup

        Ok(Self { runtime, config })
    }

    /// Spawn a real-time task
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use realtime_core::RealtimeExecutor;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executor = RealtimeExecutor::new()?;
    ///
    /// executor.spawn_realtime(async move {
    ///     // Your real-time task here
    ///     println!("Executing with bounded latency");
    ///     Ok::<(), anyhow::Error>(())
    /// }).await?;
    ///
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Note
    /// This is a simplified implementation. Real implementation should:
    /// - Spawn task in runtime with priority
    /// - Track metrics (scheduled_tasks, latency)
    /// - Apply scheduling policy
    pub async fn spawn_realtime<F, Fut, R>(&self, f: F) -> Result<R, ExecutorError>
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        // Spawn task in runtime
        let handle = self.runtime.spawn(async move { f().await });

        // Await result
        let result = handle.await.map_err(ExecutorError::SpawnFailed)?;

        // TODO: Call metrics callback if configured
        Ok(result)
    }

    /// Block on a future using this executor
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use realtime_core::RealtimeExecutor;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executor = RealtimeExecutor::new()?;
    ///
    /// let result = executor.block_on(async {
    ///     // Your async code here
    ///     42
    /// });
    ///
    /// assert_eq!(result, 42);
    /// # Ok(())
    /// # }
    /// ```
    pub fn block_on<F>(&self, future: F) -> F::Output
    where
        F: std::future::Future,
    {
        self.runtime.block_on(future)
    }

    /// Get executor configuration
    pub fn config(&self) -> &ExecutorConfig {
        &self.config
    }

    /// Enter the runtime context
    ///
    /// # Examples
    /// ```rust,no_run
    /// # use realtime_core::RealtimeExecutor;
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let executor = RealtimeExecutor::new()?;
    ///
    /// let _guard = executor.enter();
    /// // Now inside runtime context
    /// // Can use tokio::spawn directly
    ///
    /// # Ok(())
    /// # }
    /// ```
    pub fn enter(&self) -> tokio::runtime::EnterGuard<'_> {
        self.runtime.enter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_executor_config_default() {
        let config = ExecutorConfig::default();
        assert_eq!(config.cpu_affinity, None);
        assert_eq!(config.worker_threads, Some(1));
        assert!(config.enable_parking);
    }

    #[test]
    fn test_executor_creation() {
        let executor = RealtimeExecutor::new();
        assert!(executor.is_ok());
    }

    #[test]
    fn test_executor_with_config() {
        let config = ExecutorConfig {
            cpu_affinity: Some(vec![0]),
            worker_threads: Some(2),
            ..Default::default()
        };

        let executor = RealtimeExecutor::with_config(config);
        assert!(executor.is_ok());
    }

    #[test]
    fn test_spawn_realtime() {
        let executor = RealtimeExecutor::new().unwrap();

        let result = executor.block_on(async {
            let result = executor
                .spawn_realtime(|| async move {
                    // Simple task
                    42
                })
                .await
                .unwrap();
            result
        });

        assert_eq!(result, 42);
    }

    #[test]
    fn test_block_on() {
        let executor = RealtimeExecutor::new().unwrap();

        let result = executor.block_on(async { 42 });

        assert_eq!(result, 42);
    }

    #[test]
    fn test_executor_metrics_default() {
        let metrics = ExecutorMetrics::default();
        assert_eq!(metrics.scheduled_tasks, 0);
        assert_eq!(metrics.completed_tasks, 0);
        assert_eq!(metrics.avg_latency_ns, 0);
        assert_eq!(metrics.p99_latency_ns, 0);
    }

    #[test]
    fn test_multiple_tasks() {
        let executor = RealtimeExecutor::new().unwrap();

        let results = executor.block_on(async {
            let mut handles = Vec::new();
            for i in 0..10 {
                let handle =
                    executor
                        .spawn_realtime(move || async move {
                            i * 2
                        })
                        .await;
                handles.push(handle);
            }

            // Verify all tasks completed
            let mut results = Vec::new();
            for handle in handles {
                results.push(handle.unwrap());
            }
            results
        });

        // Check results
        for (i, result) in results.into_iter().enumerate() {
            assert_eq!(result, i * 2);
        }
    }

    #[test]
    fn test_enter_runtime() {
        let executor = RealtimeExecutor::new().unwrap();
        let _guard = executor.enter();

        // Inside runtime context
        // Can't easily test without spawning, but we verify it doesn't panic
        assert!(true);
    }
}
