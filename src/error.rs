//! Error types for realtime-core
//!
//! All operations in realtime-core return `Result` types to ensure
//! timing violations never panic. Errors are handled gracefully and
//! can be acted upon by the caller.

use std::io;

/// Errors that can occur with Timer operations
#[derive(thiserror::Error, Debug)]
pub enum TimerError {
    /// Invalid rate specified (must be > 0)
    #[error("Invalid rate: {0} Hz (must be > 0)")]
    InvalidRate(f64),

    /// Timer creation failed
    #[error("Timer creation failed: {0}")]
    CreationFailed(#[source] io::Error),

    /// Timer wait operation failed
    #[error("Timer wait failed: {0}")]
    Io(#[from] io::Error),

    /// Requested backend is not supported
    #[error("Backend not supported: {0}")]
    BackendNotSupported(&'static str),

    /// Feature not enabled
    #[error("Feature not enabled: {0}")]
    FeatureNotEnabled(&'static str),
}

/// Errors that can occur with Scheduler operations
#[derive(thiserror::Error, Debug)]
pub enum SchedulerError {
    /// Invalid deadline parameters
    #[error("Invalid deadline parameters: {0}")]
    InvalidDeadlineParams(&'static str),

    /// CPU affinity not supported on this platform
    #[error("CPU affinity not supported")]
    CpuAffinityNotSupported,

    /// Permission denied (CAP_SYS_NICE required for SCHED_DEADLINE)
    #[error("Permission denied (CAP_SYS_NICE required)")]
    PermissionDenied,

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Invalid CPU core specified
    #[error("Invalid CPU core: {0}")]
    InvalidCpuCore(usize),

    /// Scheduling policy not supported
    #[error("Scheduling policy not supported: {0}")]
    PolicyNotSupported(&'static str),
}

/// Errors that can occur with Executor operations
#[derive(thiserror::Error, Debug)]
pub enum ExecutorError {
    /// Executor creation failed
    #[error("Executor creation failed: {0}")]
    CreationFailed(#[source] io::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Task spawn failed
    #[error("Task spawn failed: {0}")]
    SpawnFailed(#[source] tokio::task::JoinError),

    /// Scheduling policy not supported
    #[error("Scheduling policy not supported: {0}")]
    PolicyNotSupported(&'static str),

    /// CPU affinity not supported
    #[error("CPU affinity not supported")]
    CpuAffinityNotSupported,

    /// Thread setup failed
    #[error("Thread setup failed: {0}")]
    ThreadSetupFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timer_error_display() {
        let err = TimerError::InvalidRate(0.0);
        assert!(err.to_string().contains("Invalid rate"));

        let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "test");
        let err = TimerError::Io(io_err);
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_scheduler_error_display() {
        let err = SchedulerError::InvalidDeadlineParams("runtime > deadline");
        assert!(err.to_string().contains("Invalid deadline parameters"));

        let err = SchedulerError::PermissionDenied;
        assert!(err.to_string().contains("CAP_SYS_NICE"));
    }

    #[test]
    fn test_executor_error_display() {
        let err = ExecutorError::PolicyNotSupported("SCHED_DEADLINE");
        assert!(err.to_string().contains("not supported"));
    }
}
