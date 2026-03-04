//! Sysvar population for SVM execution.
//!
//! Maps Monmouth's `BlockContext` fields into Solana sysvars (Clock, Rent,
//! EpochSchedule) so SVM programs see consistent block-level state.

use solana_clock::Clock;
use solana_epoch_schedule::EpochSchedule;
use solana_rent::Rent;

use crate::SvmError;

/// Default epoch length for Monmouth's SVM.
///
/// Monmouth uses a single continuous epoch (no leader rotation),
/// so this is set very large.
pub const DEFAULT_EPOCH_LENGTH: u64 = u64::MAX;

/// Populate a Clock sysvar from block context fields.
///
/// - `slot` = block height (Monmouth has 1:1 slot:block mapping)
/// - `unix_timestamp` = block timestamp
/// - `epoch` = height / epoch_length
pub fn clock_from_block(
    block_height: u64,
    block_timestamp: u64,
    epoch_length: u64,
) -> Result<Clock, SvmError> {
    if epoch_length == 0 {
        return Err(SvmError::Sysvar("epoch_length must be > 0".into()));
    }
    let epoch = if epoch_length == u64::MAX { 0 } else { block_height / epoch_length };

    Ok(Clock {
        slot: block_height,
        epoch_start_timestamp: (block_timestamp as i64)
            .saturating_sub((block_height % epoch_length) as i64),
        epoch,
        leader_schedule_epoch: epoch.saturating_add(1),
        unix_timestamp: block_timestamp as i64,
    })
}

/// Create Monmouth's default Rent sysvar.
///
/// Monmouth SVM uses rent-exempt-only mode: all accounts must be
/// rent-exempt. This matches Solana's current behavior post-rent-removal.
pub fn default_rent() -> Rent {
    Rent::default()
}

/// Create Monmouth's default EpochSchedule.
///
/// Single continuous epoch — no warmup, no leader rotation.
pub fn default_epoch_schedule() -> EpochSchedule {
    EpochSchedule {
        slots_per_epoch: DEFAULT_EPOCH_LENGTH,
        leader_schedule_slot_offset: DEFAULT_EPOCH_LENGTH,
        warmup: false,
        first_normal_epoch: 0,
        first_normal_slot: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_basic() {
        let clock = clock_from_block(100, 1_700_000_000, DEFAULT_EPOCH_LENGTH).unwrap();
        assert_eq!(clock.slot, 100);
        assert_eq!(clock.unix_timestamp, 1_700_000_000);
        assert_eq!(clock.epoch, 0);
    }

    #[test]
    fn clock_epoch_calculation() {
        let clock = clock_from_block(250, 1_700_000_000, 100).unwrap();
        assert_eq!(clock.epoch, 2);
    }

    #[test]
    fn clock_zero_epoch_length_fails() {
        let result = clock_from_block(0, 0, 0);
        assert!(result.is_err());
    }

    #[test]
    fn rent_is_default() {
        let rent = default_rent();
        assert_eq!(rent, Rent::default());
    }

    #[test]
    fn epoch_schedule_no_warmup() {
        let schedule = default_epoch_schedule();
        assert!(!schedule.warmup);
        assert_eq!(schedule.first_normal_epoch, 0);
    }
}
