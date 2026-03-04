//! Thread-safe coordination registry with job lifecycle and escrow.

use std::{
    collections::HashMap,
    sync::Arc,
};

use alloy_primitives::B256;
use monmouth_agent_types::{AgentId, EscrowEntry, Job, JobId, JobStatus};
use parking_lot::RwLock;
use tracing::{debug, info};

use crate::CoordinationError;

/// Default maximum number of jobs the registry will hold.
pub const DEFAULT_MAX_JOBS: usize = 50_000;

/// Internal state protected by the lock.
#[derive(Debug)]
struct Inner {
    /// Primary index: job ID to job.
    jobs: HashMap<JobId, Job>,
    /// Secondary index: proposer to job IDs.
    by_proposer: HashMap<AgentId, Vec<JobId>>,
    /// Secondary index: executor to job IDs.
    by_executor: HashMap<AgentId, Vec<JobId>>,
    /// Escrow entries by job ID.
    escrow: HashMap<JobId, EscrowEntry>,
}

/// Thread-safe coordination registry.
///
/// Manages the full lifecycle of coordination jobs between agents:
/// `Proposed → Accepted → Executing → Completed → Settled`
/// with branches to `Disputed → Settled` and `Cancelled`.
#[derive(Debug, Clone)]
pub struct CoordinationRegistry {
    inner: Arc<RwLock<Inner>>,
    max_jobs: usize,
}

impl Default for CoordinationRegistry {
    fn default() -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner {
                jobs: HashMap::new(),
                by_proposer: HashMap::new(),
                by_executor: HashMap::new(),
                escrow: HashMap::new(),
            })),
            max_jobs: DEFAULT_MAX_JOBS,
        }
    }
}

impl CoordinationRegistry {
    /// Create a new, empty coordination registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum number of jobs the registry will hold.
    #[must_use]
    pub const fn with_max_jobs(mut self, max: usize) -> Self {
        self.max_jobs = max;
        self
    }

    /// Propose a new job.
    ///
    /// The job must have status `Proposed` and no executor set.
    ///
    /// # Errors
    ///
    /// Returns an error if the registry is at capacity or the job ID is a
    /// duplicate.
    pub fn propose(&self, job: Job) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        if inner.jobs.len() >= self.max_jobs {
            return Err(CoordinationError::CapacityExceeded(self.max_jobs));
        }

        if inner.jobs.contains_key(&job.id) {
            return Err(CoordinationError::DuplicateJob(job.id));
        }

        info!(job_id = %job.id, proposer = %job.proposer, "Job proposed");

        let job_id = job.id;
        let proposer = job.proposer;
        inner.by_proposer.entry(proposer).or_default().push(job_id);
        inner.jobs.insert(job_id, job);

        Ok(())
    }

    /// Accept a proposed job, assigning an executor.
    ///
    /// Transitions the job from `Proposed` to `Accepted` and creates an
    /// escrow entry locking the payment amount.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is not found, not in `Proposed` status,
    /// or the executor is the proposer.
    pub fn accept(
        &self,
        job_id: JobId,
        executor: AgentId,
    ) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Proposed {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Accepted,
            });
        }

        info!(job_id = %job_id, executor = %executor, "Job accepted");

        job.status = JobStatus::Accepted;
        job.executor = Some(executor);

        // Create escrow entry.
        let escrow = EscrowEntry {
            job_id,
            holder: job.proposer,
            amount: job.payment_wei,
            released: false,
        };
        job.escrow_held = job.payment_wei;

        inner.by_executor.entry(executor).or_default().push(job_id);
        inner.escrow.insert(job_id, escrow);

        Ok(())
    }

    /// Mark a job as executing.
    ///
    /// Only the assigned executor can start execution.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is not in `Accepted` status or the caller
    /// is not the executor.
    pub fn start_execution(
        &self,
        job_id: JobId,
        executor: AgentId,
    ) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Accepted {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Executing,
            });
        }

        if job.executor != Some(executor) {
            return Err(CoordinationError::Unauthorized { job_id, agent: executor });
        }

        debug!(job_id = %job_id, "Job execution started");
        job.status = JobStatus::Executing;

        Ok(())
    }

    /// Complete a job with a result hash.
    ///
    /// Only the assigned executor can complete the job.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is not in `Executing` status or the caller
    /// is not the executor.
    pub fn complete(
        &self,
        job_id: JobId,
        executor: AgentId,
        result_hash: B256,
    ) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Executing {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Completed,
            });
        }

        if job.executor != Some(executor) {
            return Err(CoordinationError::Unauthorized { job_id, agent: executor });
        }

        info!(job_id = %job_id, result_hash = %result_hash, "Job completed");
        job.status = JobStatus::Completed;
        job.result_hash = Some(result_hash);

        Ok(())
    }

    /// Dispute a completed job.
    ///
    /// Only the proposer can dispute a job.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is not in `Completed` status or the caller
    /// is not the proposer.
    pub fn dispute(
        &self,
        job_id: JobId,
        proposer: AgentId,
    ) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Completed {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Disputed,
            });
        }

        if job.proposer != proposer {
            return Err(CoordinationError::Unauthorized { job_id, agent: proposer });
        }

        info!(job_id = %job_id, "Job disputed");
        job.status = JobStatus::Disputed;

        Ok(())
    }

    /// Settle a job, releasing escrow funds.
    ///
    /// Can settle from `Completed` (normal) or `Disputed` (resolution).
    /// The proposer or a resolver must call this.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is not in a settleable state.
    pub fn settle(
        &self,
        job_id: JobId,
        caller: AgentId,
    ) -> Result<EscrowEntry, CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Completed && job.status != JobStatus::Disputed {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Settled,
            });
        }

        if job.proposer != caller {
            return Err(CoordinationError::Unauthorized { job_id, agent: caller });
        }

        info!(job_id = %job_id, "Job settled");
        job.status = JobStatus::Settled;

        // Copy fields we need before releasing the borrow.
        let proposer = job.proposer;
        let escrow_held = job.escrow_held;

        // Release escrow.
        if let Some(escrow) = inner.escrow.get_mut(&job_id) {
            escrow.released = true;
            return Ok(escrow.clone());
        }

        Ok(EscrowEntry {
            job_id,
            holder: proposer,
            amount: escrow_held,
            released: true,
        })
    }

    /// Cancel a proposed or accepted job.
    ///
    /// Only the proposer can cancel. If accepted, the escrow is returned.
    ///
    /// # Errors
    ///
    /// Returns an error if the job is past the cancellable states.
    pub fn cancel(
        &self,
        job_id: JobId,
        proposer: AgentId,
    ) -> Result<(), CoordinationError> {
        let mut inner = self.inner.write();

        let job = inner
            .jobs
            .get_mut(&job_id)
            .ok_or(CoordinationError::JobNotFound(job_id))?;

        if job.status != JobStatus::Proposed && job.status != JobStatus::Accepted {
            return Err(CoordinationError::InvalidTransition {
                job_id,
                from: job.status,
                to: JobStatus::Cancelled,
            });
        }

        if job.proposer != proposer {
            return Err(CoordinationError::Unauthorized { job_id, agent: proposer });
        }

        info!(job_id = %job_id, "Job cancelled");
        job.status = JobStatus::Cancelled;

        // Release any escrow.
        if let Some(escrow) = inner.escrow.get_mut(&job_id) {
            escrow.released = true;
        }

        Ok(())
    }

    /// Look up a job by ID.
    #[must_use]
    pub fn get(&self, job_id: JobId) -> Option<Job> {
        self.inner.read().jobs.get(&job_id).cloned()
    }

    /// Get the escrow entry for a job.
    #[must_use]
    pub fn get_escrow(&self, job_id: JobId) -> Option<EscrowEntry> {
        self.inner.read().escrow.get(&job_id).cloned()
    }

    /// List jobs proposed by an agent.
    #[must_use]
    pub fn list_by_proposer(&self, proposer: AgentId) -> Vec<Job> {
        let inner = self.inner.read();
        inner
            .by_proposer
            .get(&proposer)
            .map(|ids| ids.iter().filter_map(|id| inner.jobs.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    /// List jobs assigned to an executor.
    #[must_use]
    pub fn list_by_executor(&self, executor: AgentId) -> Vec<Job> {
        let inner = self.inner.read();
        inner
            .by_executor
            .get(&executor)
            .map(|ids| ids.iter().filter_map(|id| inner.jobs.get(id).cloned()).collect())
            .unwrap_or_default()
    }

    /// Returns the total number of jobs in the registry.
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.read().jobs.len()
    }

    /// Returns `true` if the registry contains no jobs.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.inner.read().jobs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{Address, U256};

    use super::*;

    fn proposer() -> AgentId {
        AgentId(Address::repeat_byte(0xAA))
    }

    fn executor() -> AgentId {
        AgentId(Address::repeat_byte(0xBB))
    }

    fn job_id(byte: u8) -> JobId {
        JobId(B256::repeat_byte(byte))
    }

    fn test_job(id: JobId, prop: AgentId) -> Job {
        Job {
            id,
            proposer: prop,
            executor: None,
            status: JobStatus::Proposed,
            description: "Test job".to_string(),
            capability_id: "compute.run".to_string(),
            payment_wei: U256::from(1_000_000u64),
            escrow_held: U256::ZERO,
            result_hash: None,
            deadline: 1_700_100_000,
            created_at: 1_700_000_000,
        }
    }

    #[test]
    fn full_lifecycle_happy_path() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        // Propose → Accept → Execute → Complete → Settle.
        reg.propose(test_job(jid, proposer())).unwrap();
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Proposed);

        reg.accept(jid, executor()).unwrap();
        let job = reg.get(jid).unwrap();
        assert_eq!(job.status, JobStatus::Accepted);
        assert_eq!(job.executor, Some(executor()));

        // Escrow should be created.
        let escrow = reg.get_escrow(jid).unwrap();
        assert!(!escrow.released);
        assert_eq!(escrow.amount, U256::from(1_000_000u64));

        reg.start_execution(jid, executor()).unwrap();
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Executing);

        let result_hash = B256::repeat_byte(0xFF);
        reg.complete(jid, executor(), result_hash).unwrap();
        let job = reg.get(jid).unwrap();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.result_hash, Some(result_hash));

        let released = reg.settle(jid, proposer()).unwrap();
        assert!(released.released);
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Settled);
    }

    #[test]
    fn dispute_and_settle() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();
        reg.start_execution(jid, executor()).unwrap();
        reg.complete(jid, executor(), B256::ZERO).unwrap();

        reg.dispute(jid, proposer()).unwrap();
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Disputed);

        // Settle after dispute.
        let released = reg.settle(jid, proposer()).unwrap();
        assert!(released.released);
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Settled);
    }

    #[test]
    fn cancel_proposed_job() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.cancel(jid, proposer()).unwrap();
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Cancelled);
    }

    #[test]
    fn cancel_accepted_job_releases_escrow() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();
        assert!(!reg.get_escrow(jid).unwrap().released);

        reg.cancel(jid, proposer()).unwrap();
        assert_eq!(reg.get(jid).unwrap().status, JobStatus::Cancelled);
        assert!(reg.get_escrow(jid).unwrap().released);
    }

    #[test]
    fn cannot_cancel_executing_job() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();
        reg.start_execution(jid, executor()).unwrap();

        let err = reg.cancel(jid, proposer()).unwrap_err();
        assert!(matches!(err, CoordinationError::InvalidTransition { .. }));
    }

    #[test]
    fn wrong_executor_cannot_start() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);
        let wrong = AgentId(Address::repeat_byte(0xCC));

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();

        let err = reg.start_execution(jid, wrong).unwrap_err();
        assert!(matches!(err, CoordinationError::Unauthorized { .. }));
    }

    #[test]
    fn wrong_executor_cannot_complete() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();
        reg.start_execution(jid, executor()).unwrap();

        let wrong = AgentId(Address::repeat_byte(0xCC));
        let err = reg.complete(jid, wrong, B256::ZERO).unwrap_err();
        assert!(matches!(err, CoordinationError::Unauthorized { .. }));
    }

    #[test]
    fn only_proposer_can_dispute() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        reg.accept(jid, executor()).unwrap();
        reg.start_execution(jid, executor()).unwrap();
        reg.complete(jid, executor(), B256::ZERO).unwrap();

        let err = reg.dispute(jid, executor()).unwrap_err();
        assert!(matches!(err, CoordinationError::Unauthorized { .. }));
    }

    #[test]
    fn duplicate_job_rejected() {
        let reg = CoordinationRegistry::new();
        let jid = job_id(1);

        reg.propose(test_job(jid, proposer())).unwrap();
        let err = reg.propose(test_job(jid, proposer())).unwrap_err();
        assert!(matches!(err, CoordinationError::DuplicateJob(_)));
    }

    #[test]
    fn capacity_exceeded() {
        let reg = CoordinationRegistry::new().with_max_jobs(1);
        reg.propose(test_job(job_id(1), proposer())).unwrap();

        let err = reg.propose(test_job(job_id(2), proposer())).unwrap_err();
        assert!(matches!(err, CoordinationError::CapacityExceeded(1)));
    }

    #[test]
    fn list_by_proposer_and_executor() {
        let reg = CoordinationRegistry::new();

        reg.propose(test_job(job_id(1), proposer())).unwrap();
        reg.propose(test_job(job_id(2), proposer())).unwrap();
        reg.accept(job_id(1), executor()).unwrap();

        assert_eq!(reg.list_by_proposer(proposer()).len(), 2);
        assert_eq!(reg.list_by_executor(executor()).len(), 1);
        assert!(reg.list_by_executor(proposer()).is_empty());
    }

    #[test]
    fn job_not_found() {
        let reg = CoordinationRegistry::new();
        assert!(reg.get(job_id(99)).is_none());

        let err = reg.accept(job_id(99), executor()).unwrap_err();
        assert!(matches!(err, CoordinationError::JobNotFound(_)));
    }

    #[test]
    fn len_and_is_empty() {
        let reg = CoordinationRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);

        reg.propose(test_job(job_id(1), proposer())).unwrap();
        assert!(!reg.is_empty());
        assert_eq!(reg.len(), 1);
    }

    #[test]
    fn thread_safety() {
        let reg = Arc::new(CoordinationRegistry::new());
        let mut handles = vec![];

        for i in 0..10u8 {
            let r = Arc::clone(&reg);
            handles.push(std::thread::spawn(move || {
                r.propose(test_job(job_id(i), proposer())).unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(reg.len(), 10);
    }

    #[test]
    fn error_codes() {
        assert_eq!(CoordinationError::JobNotFound(job_id(1)).code(), -32860);
        assert_eq!(CoordinationError::DuplicateJob(job_id(1)).code(), -32861);
        assert_eq!(
            CoordinationError::InvalidTransition {
                job_id: job_id(1),
                from: JobStatus::Proposed,
                to: JobStatus::Completed,
            }
            .code(),
            -32862
        );
        assert_eq!(CoordinationError::CapacityExceeded(10).code(), -32864);
    }
}
