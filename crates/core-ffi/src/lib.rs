use crabcheck_core::{AlarmLevel, AlarmSpec, CoreStore};
use thiserror::Error;

uniffi::setup_scaffolding!();

#[derive(Error, Debug, uniffi::Error)]
pub enum FfiError {
    #[error("Storage error: {reason}")]
    Storage { reason: String },
}

impl From<crabcheck_core::CoreError> for FfiError {
    fn from(err: crabcheck_core::CoreError) -> Self {
        Self::Storage {
            reason: err.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiAlarmDto {
    pub id: String,
    pub title: String,
    pub level: i32,
    pub target_label: String,
    pub trigger_at_epoch_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiOutcomeDto {
    pub id: i64,
    pub alarm_id: String,
    pub outcome: String,
    pub recorded_at_epoch_ms: i64,
}

#[uniffi::export]
pub fn ffi_upsert_alarm(db_path: String, alarm: FfiAlarmDto) -> Result<(), FfiError> {
    let store = CoreStore::open(&db_path)?;
    let spec = AlarmSpec {
        id: alarm.id,
        title: alarm.title,
        level: AlarmLevel::from_i64(i64::from(alarm.level)),
        target_label: alarm.target_label,
        trigger_at_epoch_ms: alarm.trigger_at_epoch_ms,
    };
    store.upsert_alarm(&spec)?;
    Ok(())
}

#[uniffi::export]
pub fn ffi_get_alarm(db_path: String, alarm_id: String) -> Result<FfiAlarmDto, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let spec = store.get_alarm(&alarm_id)?;
    Ok(FfiAlarmDto {
        id: spec.id,
        title: spec.title,
        level: spec.level as i32,
        target_label: spec.target_label,
        trigger_at_epoch_ms: spec.trigger_at_epoch_ms,
    })
}

#[uniffi::export]
pub fn ffi_record_outcome(
    db_path: String,
    alarm_id: String,
    outcome: String,
    recorded_at_epoch_ms: i64,
) -> Result<(), FfiError> {
    let store = CoreStore::open(&db_path)?;
    store.record_outcome(&alarm_id, &outcome, recorded_at_epoch_ms)?;
    Ok(())
}

#[uniffi::export]
pub fn ffi_list_outcomes(
    db_path: String,
    alarm_id: String,
) -> Result<Vec<FfiOutcomeDto>, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let records = store.list_outcomes(&alarm_id)?;
    Ok(records
        .into_iter()
        .map(|r| FfiOutcomeDto {
            id: r.id,
            alarm_id: r.alarm_id,
            outcome: r.outcome,
            recorded_at_epoch_ms: r.recorded_at_epoch_ms,
        })
        .collect())
}
