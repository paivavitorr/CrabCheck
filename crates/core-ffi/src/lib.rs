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
    pub hour: u8,
    pub minute: u8,
    pub weekdays_mask: u8,
    pub grace_window_minutes: u16,
    pub enabled: bool,
    pub timezone: String,
    pub trigger_at_epoch_ms: i64,
    pub cooldown_unlock_at_epoch_ms: Option<i64>,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
}

impl From<AlarmSpec> for FfiAlarmDto {
    fn from(spec: AlarmSpec) -> Self {
        Self {
            id: spec.id,
            title: spec.title,
            level: spec.level.as_i32(),
            target_label: spec.target_label,
            hour: spec.hour,
            minute: spec.minute,
            weekdays_mask: spec.weekdays_mask,
            grace_window_minutes: spec.grace_window_minutes,
            enabled: spec.enabled,
            timezone: spec.timezone,
            trigger_at_epoch_ms: spec.trigger_at_epoch_ms,
            cooldown_unlock_at_epoch_ms: spec.cooldown_unlock_at_epoch_ms,
            created_at_epoch_ms: spec.created_at_epoch_ms,
            updated_at_epoch_ms: spec.updated_at_epoch_ms,
        }
    }
}

impl From<FfiAlarmDto> for AlarmSpec {
    fn from(alarm: FfiAlarmDto) -> Self {
        Self {
            id: alarm.id,
            title: alarm.title,
            level: AlarmLevel::from_i64(i64::from(alarm.level)),
            target_label: alarm.target_label,
            hour: alarm.hour,
            minute: alarm.minute,
            weekdays_mask: alarm.weekdays_mask,
            grace_window_minutes: alarm.grace_window_minutes,
            enabled: alarm.enabled,
            timezone: alarm.timezone,
            trigger_at_epoch_ms: alarm.trigger_at_epoch_ms,
            cooldown_unlock_at_epoch_ms: alarm.cooldown_unlock_at_epoch_ms,
            created_at_epoch_ms: alarm.created_at_epoch_ms,
            updated_at_epoch_ms: alarm.updated_at_epoch_ms,
            deleted_at_epoch_ms: None,
        }
    }
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
    let spec: AlarmSpec = alarm.into();
    store.upsert_alarm(&spec)?;
    Ok(())
}

#[uniffi::export]
pub fn ffi_get_alarm(db_path: String, alarm_id: String) -> Result<FfiAlarmDto, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let spec = store.get_alarm(&alarm_id)?;
    Ok(spec.into())
}

#[uniffi::export]
pub fn ffi_list_alarms(db_path: String) -> Result<Vec<FfiAlarmDto>, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let specs = store.list_alarms()?;
    Ok(specs.into_iter().map(Into::into).collect())
}

#[uniffi::export]
pub fn ffi_delete_alarm(
    db_path: String,
    alarm_id: String,
    now_epoch_ms: i64,
) -> Result<(), FfiError> {
    let store = CoreStore::open(&db_path)?;
    store.delete_alarm(&alarm_id, now_epoch_ms)?;
    Ok(())
}

#[uniffi::export]
pub fn ffi_request_cooldown_unlock(
    db_path: String,
    alarm_id: String,
    now_epoch_ms: i64,
) -> Result<i64, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let unlock_at = store.request_cooldown_unlock(&alarm_id, now_epoch_ms)?;
    Ok(unlock_at)
}

#[uniffi::export]
pub fn ffi_reschedule_after_trigger(
    db_path: String,
    alarm_id: String,
    now_epoch_ms: i64,
) -> Result<Option<i64>, FfiError> {
    let store = CoreStore::open(&db_path)?;
    let next = store.reschedule_after_trigger(&alarm_id, now_epoch_ms)?;
    Ok(next)
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
