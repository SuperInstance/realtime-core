//! Real-time scheduler configuration
//!
//! Provides CPU isolation and real-time scheduling policies (SCHED_FIFO,
//! SCHED_RR, SCHED_DEADLINE) for deterministic task execution.

use crate::error::SchedulerError;

/// Real-time scheduling policies
#[derive(Debug, Clone, Copy)]
pub enum SchedulingPolicy {
    /// SCHED_FIFO: First-in-first-out real-time policy
    /// Priority range: 1-99 (higher = more priority)
    Fifo(i32),

    /// SCHED_RR: Round-robin real-time policy
    /// Priority range: 1-99 (higher = more priority)
    RoundRobin(i32),

    /// SCHED_DEADLINE: Deadline scheduling (requires CAP_SYS_NICE)
    /// Enforces: runtime_ns ≤ deadline_ns ≤ period_ns
    Deadline {
        /// CPU time per period (ns)
        runtime_ns: u64,
        /// Maximum completion time (ns)
        deadline_ns: u64,
        /// Recurrence interval (ns)
        period_ns: u64,
    },

    /// Standard Linux scheduling (fallback)
    Other,
}

/// SCHED_DEADLINE parameters
///
/// # Invariant
/// ```text
/// runtime_ns ≤ deadline_ns ≤ period_ns
/// ```
/// This is enforced by the kernel; violation causes EBUSY.
#[derive(Debug, Clone, Copy)]
pub struct DeadlineParams {
    /// CPU time per period (ns)
    pub runtime_ns: u64,
    /// Maximum completion time (ns)
    pub deadline_ns: u64,
    /// Recurrence interval (ns)
    pub period_ns: u64,
}

impl DeadlineParams {
    /// Create new deadline parameters
    ///
    /// # Errors
    /// Returns `SchedulerError::InvalidDeadlineParams` if invariants violated
    ///
    /// # Examples
    /// ```
    /// use realtime_core::DeadlineParams;
    ///
    /// // Valid: 1ms runtime, 500µs deadline, 500ms period
    /// let params = DeadlineParams::new(1_000_000, 500_000, 500_000_000);
    /// assert!(params.is_ok());
    ///
    /// // Invalid: runtime > deadline
    /// let params = DeadlineParams::new(500_000, 1_000_000, 500_000_000);
    /// assert!(params.is_err());
    /// ```
    pub fn new(runtime_ns: u64, deadline_ns: u64, period_ns: u64) -> Result<Self, SchedulerError> {
        // Enforce invariant: runtime ≤ deadline ≤ period
        if runtime_ns > deadline_ns {
            return Err(SchedulerError::InvalidDeadlineParams(
                "runtime_ns must be ≤ deadline_ns",
            ));
        }
        if deadline_ns > period_ns {
            return Err(SchedulerError::InvalidDeadlineParams(
                "deadline_ns must be ≤ period_ns",
            ));
        }

        Ok(Self {
            runtime_ns,
            deadline_ns,
            period_ns,
        })
    }

    /// Validate invariants
    ///
    /// # Examples
    /// ```
    /// use realtime_core::DeadlineParams;
    ///
    /// let params = DeadlineParams::new(1_000_000, 500_000, 500_000_000).unwrap();
    /// assert!(params.validate().is_ok());
    /// ```
    pub fn validate(&self) -> Result<(), SchedulerError> {
        if self.runtime_ns > self.deadline_ns {
            return Err(SchedulerError::InvalidDeadlineParams(
                "runtime_ns must be ≤ deadline_ns",
            ));
        }
        if self.deadline_ns > self.period_ns {
            return Err(SchedulerError::InvalidDeadlineParams(
                "deadline_ns must be ≤ period_ns",
            ));
        }
        Ok(())
    }
}

/// Real-time scheduler configuration
///
/// # Invariants
/// - Only one task should run on an isolated CPU
/// - SCHED_DEADLINE parameters must satisfy: runtime ≤ deadline ≤ period
/// - CPU affinity must not include isolated cores for non-RT tasks
///
/// # Examples
/// ```
/// use realtime_core::{Scheduler, SchedulingPolicy};
///
/// let scheduler = Scheduler::new().unwrap();
/// ```
pub struct Scheduler {
    /// CPU cores to run on (for isolation)
    cpu_affinity: Option<Vec<usize>>,
    /// Scheduling policy
    scheduling_policy: SchedulingPolicy,
    /// Deadline parameters (if using SCHED_DEADLINE)
    deadline_params: Option<DeadlineParams>,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

impl Scheduler {
    /// Create a new scheduler with default settings
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Scheduler;
    ///
    /// let scheduler = Scheduler::new().unwrap();
    /// assert_eq!(scheduler.cpu_affinity(), None);
    /// ```
    pub fn new() -> Result<Self, SchedulerError> {
        Ok(Self {
            cpu_affinity: None,
            scheduling_policy: SchedulingPolicy::Other,
            deadline_params: None,
        })
    }

    /// Set CPU affinity to isolate this scheduler's tasks
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Scheduler;
    ///
    /// let mut scheduler = Scheduler::new().unwrap();
    /// scheduler.set_cpu_affinity(vec![1]); // Isolate to CPU 1
    /// assert_eq!(scheduler.cpu_affinity(), Some(&vec![1]));
    /// ```
    pub fn set_cpu_affinity(&mut self, cpus: Vec<usize>) -> Result<(), SchedulerError> {
        // Validate CPU cores
        for &cpu in &cpus {
            if cpu >= num_cpus::get() {
                return Err(SchedulerError::InvalidCpuCore(cpu));
            }
        }

        // TODO: Verify cpus are isolated (check /sys/devices/system/cpu/isolated)
        self.cpu_affinity = Some(cpus);
        Ok(())
    }

    /// Get CPU affinity
    pub fn cpu_affinity(&self) -> Option<&[usize]> {
        self.cpu_affinity.as_deref()
    }

    /// Set SCHED_FIFO policy
    ///
    /// # Examples
    /// ```
    /// use realtime_core::{Scheduler, SchedulingPolicy};
    ///
    /// let mut scheduler = Scheduler::new().unwrap();
    /// scheduler.set_fifo(50); // Priority 50
    /// ```
    pub fn set_fifo(&mut self, priority: i32) -> Result<(), SchedulerError> {
        if !(1..=99).contains(&priority) {
            return Err(SchedulerError::InvalidDeadlineParams(
                "FIFO priority must be 1-99",
            ));
        }

        self.scheduling_policy = SchedulingPolicy::Fifo(priority);
        self.deadline_params = None;
        Ok(())
    }

    /// Set SCHED_RR policy
    ///
    /// # Examples
    /// ```
    /// use realtime_core::{Scheduler, SchedulingPolicy};
    ///
    /// let mut scheduler = Scheduler::new().unwrap();
    /// scheduler.set_round_robin(50); // Priority 50
    /// ```
    pub fn set_round_robin(&mut self, priority: i32) -> Result<(), SchedulerError> {
        if !(1..=99).contains(&priority) {
            return Err(SchedulerError::InvalidDeadlineParams(
                "Round-robin priority must be 1-99",
            ));
        }

        self.scheduling_policy = SchedulingPolicy::RoundRobin(priority);
        self.deadline_params = None;
        Ok(())
    }

    /// Apply SCHED_DEADLINE parameters
    ///
    /// # Invariant
    /// ```text
    /// runtime_ns ≤ deadline_ns ≤ period_ns
    /// ```
    /// This is enforced by the kernel; violation causes EBUSY.
    ///
    /// # Examples
    /// ```
    /// use realtime_core::Scheduler;
    ///
    /// let mut scheduler = Scheduler::new().unwrap();
    /// // Runtime: 1ms, Deadline: 500µs, Period: 500ms (2 Hz)
    /// scheduler.set_deadline(1_000_000, 500_000, 500_000_000).unwrap();
    /// ```
    pub fn set_deadline(
        &mut self,
        runtime_ns: u64,
        deadline_ns: u64,
        period_ns: u64,
    ) -> Result<(), SchedulerError> {
        let params = DeadlineParams::new(runtime_ns, deadline_ns, period_ns)?;

        self.deadline_params = Some(params);
        self.scheduling_policy = SchedulingPolicy::Deadline {
            runtime_ns,
            deadline_ns,
            period_ns,
        };

        Ok(())
    }

    /// Get scheduling policy
    pub fn scheduling_policy(&self) -> &SchedulingPolicy {
        &self.scheduling_policy
    }

    /// Get deadline parameters (if using SCHED_DEADLINE)
    pub fn deadline_params(&self) -> Option<DeadlineParams> {
        self.deadline_params
    }

    /// Apply scheduling policy to current thread
    ///
    /// # Examples
    /// ```rust,no_run
    /// use realtime_core::Scheduler;
    ///
    /// let mut scheduler = Scheduler::new().unwrap();
    /// scheduler.set_fifo(50).unwrap();
    /// scheduler.apply_to_current_thread().unwrap();
    /// ```
    ///
    /// # Note
    /// This is a stub implementation. Real implementation requires:
    /// - libc::sched_setscheduler for SCHED_FIFO/SCHED_RR
    /// - libc::syscall(SYS_sched_setattr) for SCHED_DEADLINE
    /// - CAP_SYS_NICE capability
    pub fn apply_to_current_thread(&self) -> Result<(), SchedulerError> {
        match &self.scheduling_policy {
            SchedulingPolicy::Fifo(_prio) => {
                // TODO: Implement sched_setscheduler
                Err(SchedulerError::PolicyNotSupported("SCHED_FIFO"))
            }
            SchedulingPolicy::RoundRobin(_prio) => {
                // TODO: Implement sched_setscheduler
                Err(SchedulerError::PolicyNotSupported("SCHED_RR"))
            }
            SchedulingPolicy::Deadline {
                runtime_ns,
                deadline_ns,
                period_ns,
            } => {
                // TODO: Implement sched_setattr
                let _ = (*runtime_ns, *deadline_ns, *period_ns);
                Err(SchedulerError::PolicyNotSupported("SCHED_DEADLINE"))
            }
            SchedulingPolicy::Other => Ok(()),
        }
    }

    /// Apply CPU affinity to current thread
    ///
    /// # Note
    /// This is a stub implementation. Real implementation requires:
    /// - libc::sched_setaffinity
    /// - pthread_setaffinity_np
    pub fn apply_cpu_affinity(&self) -> Result<(), SchedulerError> {
        if let Some(cpus) = &self.cpu_affinity {
            // TODO: Implement sched_setaffinity
            let _ = cpus;
            Err(SchedulerError::CpuAffinityNotSupported)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = Scheduler::new().unwrap();
        assert_eq!(scheduler.cpu_affinity(), None);
    }

    #[test]
    fn test_set_cpu_affinity() {
        let mut scheduler = Scheduler::new().unwrap();

        // Valid CPU
        assert!(scheduler.set_cpu_affinity(vec![0]).is_ok());
        assert_eq!(scheduler.cpu_affinity().map(|v| v.to_vec()), Some(vec![0]));

        // Invalid CPU (likely more than system has)
        assert!(scheduler.set_cpu_affinity(vec![999999]).is_err());
    }

    #[test]
    fn test_set_fifo() {
        let mut scheduler = Scheduler::new().unwrap();

        // Valid priorities
        assert!(scheduler.set_fifo(1).is_ok());
        assert!(scheduler.set_fifo(50).is_ok());
        assert!(scheduler.set_fifo(99).is_ok());

        // Invalid priorities
        assert!(scheduler.set_fifo(0).is_err());
        assert!(scheduler.set_fifo(100).is_err());
    }

    #[test]
    fn test_set_round_robin() {
        let mut scheduler = Scheduler::new().unwrap();

        // Valid priorities
        assert!(scheduler.set_round_robin(1).is_ok());
        assert!(scheduler.set_round_robin(50).is_ok());
        assert!(scheduler.set_round_robin(99).is_ok());

        // Invalid priorities
        assert!(scheduler.set_round_robin(0).is_err());
        assert!(scheduler.set_round_robin(100).is_err());
    }

    #[test]
    fn test_set_deadline_valid() {
        let mut scheduler = Scheduler::new().unwrap();

        // Valid: 500µs runtime, 1ms deadline, 500ms period
        assert!(scheduler
            .set_deadline(500_000, 1_000_000, 500_000_000)
            .is_ok());

        // Valid: equal values
        assert!(scheduler
            .set_deadline(1_000_000, 1_000_000, 1_000_000)
            .is_ok());
    }

    #[test]
    fn test_set_deadline_invalid() {
        let mut scheduler = Scheduler::new().unwrap();

        // Invalid: runtime > deadline
        assert!(scheduler
            .set_deadline(1_000_000, 500_000, 500_000_000)
            .is_err());

        // Invalid: deadline > period
        assert!(scheduler
            .set_deadline(1_000_000, 500_000_000, 1_000_000)
            .is_err());
    }

    #[test]
    fn test_deadline_params_validation() {
        // Valid parameters: 500µs runtime, 1ms deadline, 500ms period
        let params = DeadlineParams::new(500_000, 1_000_000, 500_000_000).unwrap();
        assert!(params.validate().is_ok());

        // Invalid: runtime > deadline
        let result = DeadlineParams::new(1_000_000, 500_000, 500_000_000);
        assert!(result.is_err());

        // Invalid: deadline > period
        let result = DeadlineParams::new(1_000_000, 500_000_000, 1_000_000);
        assert!(result.is_err());
    }

    #[test]
    fn test_scheduler_default() {
        let scheduler = Scheduler::default();
        assert_eq!(scheduler.cpu_affinity(), None);
    }
}
