//! Error types for the coordination module.

use monmouth_agent_types::{AgentId, JobId, JobStatus};

/// Errors that can occur during coordination operations.
#[derive(Debug, Clone, thiserror::Error)]
pub enum CoordinationError {
    /// No job found with the given ID.
    #[error("job not found: {0}")]
    JobNotFound(JobId),

    /// A job with this ID already exists.
    #[error("duplicate job: {0}")]
    DuplicateJob(JobId),

    /// Invalid state transition for a job.
    #[error("invalid transition for job {job_id}: {from:?} -> {to:?}")]
    InvalidTransition {
        /// The job that was being transitioned.
        job_id: JobId,
        /// The current status.
        from: JobStatus,
        /// The attempted target status.
        to: JobStatus,
    },

    /// The caller is not authorised for this operation.
    #[error("unauthorised: agent {agent} cannot perform this action on job {job_id}")]
    Unauthorized {
        /// The job being acted upon.
        job_id: JobId,
        /// The agent that attempted the action.
        agent: AgentId,
    },

    /// The registry is at capacity.
    #[error("coordination registry capacity exceeded (max: {0})")]
    CapacityExceeded(usize),

    /// The job has passed its deadline.
    #[error("job {0} has passed its deadline")]
    DeadlineExpired(JobId),
}

impl CoordinationError {
    /// Returns the error code for JSON-RPC responses.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::JobNotFound(_) => -32860,
            Self::DuplicateJob(_) => -32861,
            Self::InvalidTransition { .. } => -32862,
            Self::Unauthorized { .. } => -32863,
            Self::CapacityExceeded(_) => -32864,
            Self::DeadlineExpired(_) => -32865,
        }
    }
}
