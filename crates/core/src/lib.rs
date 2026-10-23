use chrono::{Datelike, Days, Local, LocalResult, NaiveTime, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const COOLDOWN_DURATION_MS: i64 = 86_400_000;
pub const DEFAULT_CONFIDENCE_THRESHOLD: f32 = 0.70;
pub const REQUIRED_CONSECUTIVE_FRAMES: u8 = 5;

pub const CURATED_OBJECT_LABELS: &[(&str, &str)] = &[
    ("Shoe", "Ténis / Calçado"),
    ("Book", "Livro"),
    ("Bottle", "Garrafa de Água"),
    ("Cup", "Caneca / Copo"),
    ("Laptop", "Computador Portátil"),
];

pub fn curated_synonyms_for(target_label: &str) -> Vec<String> {
    let norm = target_label.trim().to_ascii_lowercase();
    let synonyms: &[&str] = match norm.as_str() {
        "shoe" => &[
            "shoe",
            "footwear",
            "sneakers",
            "boot",
            "sandal",
            "sports equipment",
        ],
        "book" => &["book", "publication", "paper", "notebook", "novel"],
        "bottle" => &[
            "bottle",
            "water bottle",
            "drinkware",
            "plastic bottle",
            "glass bottle",
        ],
        "cup" => &["cup", "mug", "coffee cup", "tableware", "drinkware", "teacup"],
        "laptop" => &[
            "laptop",
            "computer",
            "personal computer",
            "netbook",
            "computer keyboard",
            "display device",
        ],
        _ => return if norm.is_empty() { Vec::new() } else { vec![norm] },
    };
    synonyms.iter().map(|s| (*s).to_string()).collect()
}

pub fn matches_curated_label(target_label: &str, detected_label: &str) -> bool {
    let detected = detected_label.trim().to_ascii_lowercase();
    if detected.is_empty() {
        return false;
    }
    curated_synonyms_for(target_label)
        .iter()
        .any(|syn| syn == &detected)
}

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Invalid alarm outcome: {0}")]
    InvalidOutcome(String),
    #[error("Alarm not found: {0}")]
    NotFound(String),
    #[error("Validation error: {0}")]
    Validation(String),
    #[error("Cooldown active until epoch ms: {unlock_at_epoch_ms}")]
    CooldownActive { unlock_at_epoch_ms: i64 },
}

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AlarmLevel {
    Passive = 1,
    Persistent = 2,
    ProofOfAction = 3,
}

impl AlarmLevel {
    pub fn from_i64(val: i64) -> Self {
        match val {
            1 => Self::Passive,
            2 => Self::Persistent,
            _ => Self::ProofOfAction,
        }
    }

    pub fn as_i32(self) -> i32 {
        self as i32
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmSpec {
    pub id: String,
    pub title: String,
    pub level: AlarmLevel,
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
    pub deleted_at_epoch_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmDraft {
    pub id: Option<String>,
    pub title: String,
    pub level: AlarmLevel,
    pub target_label: String,
    pub hour: u8,
    pub minute: u8,
    pub weekdays_mask: u8,
    pub grace_window_minutes: u16,
    pub enabled: bool,
    pub timezone: String,
}

impl AlarmSpec {
    pub fn new_from_draft(
        draft: AlarmDraft,
        now_epoch_ms: i64,
        existing: Option<&Self>,
    ) -> Result<Self> {
        let trigger_at_epoch_ms = compute_next_trigger_ms(
            draft.hour,
            draft.minute,
            draft.weekdays_mask,
            now_epoch_ms,
        )?;

        let id = draft
            .id
            .or_else(|| existing.map(|e| e.id.clone()))
            .unwrap_or_else(|| Uuid::now_v7().to_string());

        let created_at_epoch_ms = existing
            .map(|e| e.created_at_epoch_ms)
            .unwrap_or(now_epoch_ms);

        let cooldown_unlock_at_epoch_ms = existing.and_then(|e| e.cooldown_unlock_at_epoch_ms);

        let spec = Self {
            id,
            title: draft.title,
            level: draft.level,
            target_label: draft.target_label,
            hour: draft.hour,
            minute: draft.minute,
            weekdays_mask: draft.weekdays_mask,
            grace_window_minutes: draft.grace_window_minutes,
            enabled: draft.enabled,
            timezone: if draft.timezone.trim().is_empty() {
                "Local".to_string()
            } else {
                draft.timezone
            },
            trigger_at_epoch_ms,
            cooldown_unlock_at_epoch_ms,
            created_at_epoch_ms,
            updated_at_epoch_ms: now_epoch_ms,
            deleted_at_epoch_ms: None,
        };

        spec.validate()?;
        Ok(spec)
    }

    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(CoreError::Validation(
                "O título do alarme não pode estar vazio".to_string(),
            ));
        }
        if self.hour > 23 {
            return Err(CoreError::Validation(format!(
                "Hora inválida: {} (esperado 0..=23)",
                self.hour
            )));
        }
        if self.minute > 59 {
            return Err(CoreError::Validation(format!(
                "Minuto inválido: {} (esperado 0..=59)",
                self.minute
            )));
        }
        if self.weekdays_mask > 0b0111_1111 {
            return Err(CoreError::Validation(format!(
                "Máscara de dias da semana inválida: {:#010b}",
                self.weekdays_mask
            )));
        }
        if !(1..=240).contains(&self.grace_window_minutes) {
            return Err(CoreError::Validation(format!(
                "Janela de cumprimento inválida: {} (esperado 1..=240 min)",
                self.grace_window_minutes
            )));
        }
        if self.level == AlarmLevel::ProofOfAction && self.target_label.trim().is_empty() {
            return Err(CoreError::Validation(
                "Alarme de Nível 3 exige um objeto de prova selecionado".to_string(),
            ));
        }
        Ok(())
    }
}

pub fn compute_next_trigger_ms(
    hour: u8,
    minute: u8,
    weekdays_mask: u8,
    now_epoch_ms: i64,
) -> Result<i64> {
    if hour > 23 {
        return Err(CoreError::Validation(format!(
            "Hora inválida: {hour} (esperado 0..=23)"
        )));
    }
    if minute > 59 {
        return Err(CoreError::Validation(format!(
            "Minuto inválido: {minute} (esperado 0..=59)"
        )));
    }
    if weekdays_mask > 0b0111_1111 {
        return Err(CoreError::Validation(format!(
            "Máscara de dias da semana inválida: {weekdays_mask:#010b}"
        )));
    }

    let now_dt = match Local.timestamp_millis_opt(now_epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => {
            return Err(CoreError::Validation(format!(
                "Timestamp inválido: {now_epoch_ms}"
            )))
        }
    };

    let target_time = NaiveTime::from_hms_opt(u32::from(hour), u32::from(minute), 0)
        .ok_or_else(|| CoreError::Validation("Horário inválido".to_string()))?;

    let max_days = if weekdays_mask == 0 { 1 } else { 7 };

    for day_offset in 0..=max_days {
        let candidate_date = now_dt
            .date_naive()
            .checked_add_days(Days::new(day_offset))
            .ok_or_else(|| CoreError::Validation("Overflow ao calcular data".to_string()))?;

        if weekdays_mask != 0 {
            let weekday_bit = 1u8 << candidate_date.weekday().num_days_from_monday();
            if (weekdays_mask & weekday_bit) == 0 {
                continue;
            }
        }

        let naive_dt = candidate_date.and_time(target_time);
        let candidate_ms = match Local.from_local_datetime(&naive_dt) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.timestamp_millis(),
            LocalResult::None => {
                let shifted = naive_dt + chrono::Duration::hours(1);
                match Local.from_local_datetime(&shifted) {
                    LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
                        dt.timestamp_millis()
                    }
                    LocalResult::None => continue,
                }
            }
        };

        if candidate_ms > now_epoch_ms {
            return Ok(candidate_ms);
        }
    }

    Err(CoreError::Validation(
        "Não foi possível calcular o próximo disparo para a máscara informada".to_string(),
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlarmOutcomeRecord {
    pub id: i64,
    pub alarm_id: String,
    pub outcome: String,
    pub recorded_at_epoch_ms: i64,
    pub proof_image_path: Option<String>,
    pub matched_label: Option<String>,
    pub confidence: Option<f32>,
}

pub fn extract_hashtags(body: &str) -> Vec<String> {
    let mut tags = Vec::new();
    for token in body.split_whitespace() {
        if let Some(rest) = token.strip_prefix('#') {
            let cleaned: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .collect::<String>()
                .to_lowercase();
            if !cleaned.is_empty() && !tags.contains(&cleaned) {
                tags.push(cleaned);
            }
        }
    }
    tags
}

pub fn format_local_time_hm(epoch_ms: i64) -> String {
    match Local.timestamp_millis_opt(epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
            dt.format("%H:%M").to_string()
        }
        LocalResult::None => epoch_ms.to_string(),
    }
}

pub fn format_local_time_hms(epoch_ms: i64) -> String {
    match Local.timestamp_millis_opt(epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => {
            dt.format("%H:%M:%S").to_string()
        }
        LocalResult::None => epoch_ms.to_string(),
    }
}

pub fn day_reference_epoch_ms(now_epoch_ms: i64, day_offset: i64) -> i64 {
    if day_offset == 0 {
        return now_epoch_ms;
    }
    let base_dt = match Local.timestamp_millis_opt(now_epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
        LocalResult::None => return now_epoch_ms + day_offset * 86_400_000,
    };
    let target_date = if day_offset > 0 {
        base_dt
            .date_naive()
            .checked_add_days(Days::new(day_offset as u64))
    } else {
        base_dt
            .date_naive()
            .checked_sub_days(Days::new((-day_offset) as u64))
    };
    if let (Some(date), Some(noon)) = (target_date, NaiveTime::from_hms_opt(12, 0, 0)) {
        let naive_dt = date.and_time(noon);
        if let LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) =
            Local.from_local_datetime(&naive_dt)
        {
            return dt.timestamp_millis();
        }
    }
    now_epoch_ms + day_offset * 86_400_000
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuickNoteRecord {
    pub id: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at_epoch_ms: i64,
    pub updated_at_epoch_ms: i64,
    pub deleted_at_epoch_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineOutcomeEntry {
    pub id: i64,
    pub alarm_id: String,
    pub alarm_title: String,
    pub alarm_level: AlarmLevel,
    pub outcome: String,
    pub recorded_at_epoch_ms: i64,
    pub proof_image_path: Option<String>,
    pub matched_label: Option<String>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DailyTimelineSnapshot {
    pub date_iso: String,
    pub day_start_epoch_ms: i64,
    pub day_end_epoch_ms: i64,
    pub scheduled_alarms: Vec<AlarmSpec>,
    pub outcomes: Vec<TimelineOutcomeEntry>,
    pub notes: Vec<QuickNoteRecord>,
}

pub struct CoreStore {
    conn: Connection,
}

impl CoreStore {
    pub fn open(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS alarms (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                level INTEGER NOT NULL,
                target_label TEXT NOT NULL,
                hour INTEGER NOT NULL DEFAULT 6,
                minute INTEGER NOT NULL DEFAULT 0,
                weekdays_mask INTEGER NOT NULL DEFAULT 0,
                grace_window_minutes INTEGER NOT NULL DEFAULT 30,
                enabled INTEGER NOT NULL DEFAULT 1,
                timezone TEXT NOT NULL DEFAULT 'Local',
                trigger_at_epoch_ms INTEGER NOT NULL,
                cooldown_unlock_at_epoch_ms INTEGER,
                created_at_epoch_ms INTEGER NOT NULL DEFAULT 0,
                updated_at_epoch_ms INTEGER NOT NULL DEFAULT 0,
                deleted_at_epoch_ms INTEGER
            );
            CREATE TABLE IF NOT EXISTS alarm_outcomes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alarm_id TEXT NOT NULL,
                outcome TEXT NOT NULL,
                recorded_at_epoch_ms INTEGER NOT NULL,
                proof_image_path TEXT,
                matched_label TEXT,
                confidence REAL
            );
            CREATE TABLE IF NOT EXISTS quick_notes (
                id TEXT PRIMARY KEY,
                body TEXT NOT NULL,
                tags_csv TEXT NOT NULL DEFAULT '',
                created_at_epoch_ms INTEGER NOT NULL,
                updated_at_epoch_ms INTEGER NOT NULL,
                deleted_at_epoch_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_alarm_outcomes_recorded_at
                ON alarm_outcomes(recorded_at_epoch_ms);
            CREATE INDEX IF NOT EXISTS idx_quick_notes_created_at
                ON quick_notes(created_at_epoch_ms);",
        )?;

        self.migrate_spike_columns_if_needed()?;
        Ok(())
    }

    fn migrate_spike_columns_if_needed(&self) -> Result<()> {
        let mut stmt = self.conn.prepare("PRAGMA table_info(alarms)")?;
        let cols: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let alter_statements = [
            ("hour", "ALTER TABLE alarms ADD COLUMN hour INTEGER NOT NULL DEFAULT 6"),
            ("minute", "ALTER TABLE alarms ADD COLUMN minute INTEGER NOT NULL DEFAULT 0"),
            (
                "weekdays_mask",
                "ALTER TABLE alarms ADD COLUMN weekdays_mask INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "grace_window_minutes",
                "ALTER TABLE alarms ADD COLUMN grace_window_minutes INTEGER NOT NULL DEFAULT 30",
            ),
            (
                "enabled",
                "ALTER TABLE alarms ADD COLUMN enabled INTEGER NOT NULL DEFAULT 1",
            ),
            (
                "timezone",
                "ALTER TABLE alarms ADD COLUMN timezone TEXT NOT NULL DEFAULT 'Local'",
            ),
            (
                "cooldown_unlock_at_epoch_ms",
                "ALTER TABLE alarms ADD COLUMN cooldown_unlock_at_epoch_ms INTEGER",
            ),
            (
                "created_at_epoch_ms",
                "ALTER TABLE alarms ADD COLUMN created_at_epoch_ms INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "updated_at_epoch_ms",
                "ALTER TABLE alarms ADD COLUMN updated_at_epoch_ms INTEGER NOT NULL DEFAULT 0",
            ),
            (
                "deleted_at_epoch_ms",
                "ALTER TABLE alarms ADD COLUMN deleted_at_epoch_ms INTEGER",
            ),
        ];

        for (col_name, sql) in alter_statements {
            if !cols.iter().any(|c| c == col_name) {
                self.conn.execute(sql, [])?;
            }
        }

        let mut out_stmt = self.conn.prepare("PRAGMA table_info(alarm_outcomes)")?;
        let out_cols: Vec<String> = out_stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<Vec<_>, _>>()?;

        let out_alters = [
            (
                "proof_image_path",
                "ALTER TABLE alarm_outcomes ADD COLUMN proof_image_path TEXT",
            ),
            (
                "matched_label",
                "ALTER TABLE alarm_outcomes ADD COLUMN matched_label TEXT",
            ),
            (
                "confidence",
                "ALTER TABLE alarm_outcomes ADD COLUMN confidence REAL",
            ),
        ];

        for (col_name, sql) in out_alters {
            if !out_cols.iter().any(|c| c == col_name) {
                self.conn.execute(sql, [])?;
            }
        }
        Ok(())
    }

    fn row_to_spec(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlarmSpec> {
        let level_raw: i64 = row.get(2)?;
        let hour_raw: i64 = row.get(4)?;
        let minute_raw: i64 = row.get(5)?;
        let mask_raw: i64 = row.get(6)?;
        let grace_raw: i64 = row.get(7)?;
        let enabled_raw: i64 = row.get(8)?;

        Ok(AlarmSpec {
            id: row.get(0)?,
            title: row.get(1)?,
            level: AlarmLevel::from_i64(level_raw),
            target_label: row.get(3)?,
            hour: u8::try_from(hour_raw).unwrap_or(0),
            minute: u8::try_from(minute_raw).unwrap_or(0),
            weekdays_mask: u8::try_from(mask_raw).unwrap_or(0),
            grace_window_minutes: u16::try_from(grace_raw).unwrap_or(30),
            enabled: enabled_raw != 0,
            timezone: row.get(9)?,
            trigger_at_epoch_ms: row.get(10)?,
            cooldown_unlock_at_epoch_ms: row.get(11)?,
            created_at_epoch_ms: row.get(12)?,
            updated_at_epoch_ms: row.get(13)?,
            deleted_at_epoch_ms: row.get(14)?,
        })
    }

    fn is_relaxing_n3(existing: &AlarmSpec, candidate: &AlarmSpec) -> bool {
        if existing.level != AlarmLevel::ProofOfAction || !existing.enabled {
            return false;
        }
        let lowered_level = (candidate.level as u8) < (existing.level as u8);
        let disabled = !candidate.enabled;
        let changed_time = candidate.hour != existing.hour || candidate.minute != existing.minute;
        let changed_target = candidate.target_label != existing.target_label;
        let removed_weekdays = (existing.weekdays_mask & !candidate.weekdays_mask) != 0;

        lowered_level || disabled || changed_time || changed_target || removed_weekdays
    }

    fn verify_n3_cooldown(existing: &AlarmSpec, now_epoch_ms: i64) -> Result<()> {
        if existing.level != AlarmLevel::ProofOfAction || !existing.enabled {
            return Ok(());
        }
        match existing.cooldown_unlock_at_epoch_ms {
            Some(unlock_at) if now_epoch_ms >= unlock_at => Ok(()),
            Some(unlock_at) => Err(CoreError::CooldownActive {
                unlock_at_epoch_ms: unlock_at,
            }),
            None => Err(CoreError::CooldownActive {
                unlock_at_epoch_ms: now_epoch_ms + COOLDOWN_DURATION_MS,
            }),
        }
    }

    pub fn request_cooldown_unlock(&self, alarm_id: &str, now_epoch_ms: i64) -> Result<i64> {
        let existing = self.get_alarm(alarm_id)?;
        let unlock_at = match existing.cooldown_unlock_at_epoch_ms {
            Some(ts) if ts > now_epoch_ms => ts,
            _ => now_epoch_ms + COOLDOWN_DURATION_MS,
        };

        self.conn.execute(
            "UPDATE alarms
             SET cooldown_unlock_at_epoch_ms = ?2, updated_at_epoch_ms = ?3
             WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
            params![alarm_id, unlock_at, now_epoch_ms],
        )?;
        Ok(unlock_at)
    }

    pub fn upsert_alarm(&self, spec: &AlarmSpec) -> Result<()> {
        spec.validate()?;

        let mut final_cooldown = spec.cooldown_unlock_at_epoch_ms;
        if let Ok(existing) = self.get_alarm(&spec.id) {
            if Self::is_relaxing_n3(&existing, spec) {
                Self::verify_n3_cooldown(&existing, spec.updated_at_epoch_ms)?;
                final_cooldown = None;
            }
        }

        self.conn.execute(
            "INSERT INTO alarms (
                id, title, level, target_label, hour, minute, weekdays_mask,
                grace_window_minutes, enabled, timezone, trigger_at_epoch_ms,
                cooldown_unlock_at_epoch_ms, created_at_epoch_ms, updated_at_epoch_ms,
                deleted_at_epoch_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                level = excluded.level,
                target_label = excluded.target_label,
                hour = excluded.hour,
                minute = excluded.minute,
                weekdays_mask = excluded.weekdays_mask,
                grace_window_minutes = excluded.grace_window_minutes,
                enabled = excluded.enabled,
                timezone = excluded.timezone,
                trigger_at_epoch_ms = excluded.trigger_at_epoch_ms,
                cooldown_unlock_at_epoch_ms = excluded.cooldown_unlock_at_epoch_ms,
                created_at_epoch_ms = excluded.created_at_epoch_ms,
                updated_at_epoch_ms = excluded.updated_at_epoch_ms,
                deleted_at_epoch_ms = excluded.deleted_at_epoch_ms",
            params![
                spec.id,
                spec.title,
                spec.level as i64,
                spec.target_label,
                i64::from(spec.hour),
                i64::from(spec.minute),
                i64::from(spec.weekdays_mask),
                i64::from(spec.grace_window_minutes),
                i64::from(spec.enabled),
                spec.timezone,
                spec.trigger_at_epoch_ms,
                final_cooldown,
                spec.created_at_epoch_ms,
                spec.updated_at_epoch_ms,
                spec.deleted_at_epoch_ms,
            ],
        )?;
        Ok(())
    }

    pub fn get_alarm(&self, alarm_id: &str) -> Result<AlarmSpec> {
        let maybe_spec = self
            .conn
            .query_row(
                "SELECT
                    id, title, level, target_label, hour, minute, weekdays_mask,
                    grace_window_minutes, enabled, timezone, trigger_at_epoch_ms,
                    cooldown_unlock_at_epoch_ms, created_at_epoch_ms, updated_at_epoch_ms,
                    deleted_at_epoch_ms
                 FROM alarms
                 WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
                params![alarm_id],
                Self::row_to_spec,
            )
            .optional()?;

        maybe_spec.ok_or_else(|| CoreError::NotFound(alarm_id.to_string()))
    }

    pub fn list_alarms(&self) -> Result<Vec<AlarmSpec>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, title, level, target_label, hour, minute, weekdays_mask,
                grace_window_minutes, enabled, timezone, trigger_at_epoch_ms,
                cooldown_unlock_at_epoch_ms, created_at_epoch_ms, updated_at_epoch_ms,
                deleted_at_epoch_ms
             FROM alarms
             WHERE deleted_at_epoch_ms IS NULL
             ORDER BY hour ASC, minute ASC, id ASC",
        )?;

        let rows = stmt.query_map([], Self::row_to_spec)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn set_alarm_enabled(
        &self,
        alarm_id: &str,
        enabled: bool,
        now_epoch_ms: i64,
    ) -> Result<AlarmSpec> {
        let mut spec = self.get_alarm(alarm_id)?;
        if !enabled {
            Self::verify_n3_cooldown(&spec, now_epoch_ms)?;
            spec.cooldown_unlock_at_epoch_ms = None;
        } else {
            spec.trigger_at_epoch_ms = compute_next_trigger_ms(
                spec.hour,
                spec.minute,
                spec.weekdays_mask,
                now_epoch_ms,
            )?;
        }
        spec.enabled = enabled;
        spec.updated_at_epoch_ms = now_epoch_ms;

        self.conn.execute(
            "UPDATE alarms
             SET enabled = ?2,
                 trigger_at_epoch_ms = ?3,
                 cooldown_unlock_at_epoch_ms = ?4,
                 updated_at_epoch_ms = ?5
             WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
            params![
                alarm_id,
                i64::from(spec.enabled),
                spec.trigger_at_epoch_ms,
                spec.cooldown_unlock_at_epoch_ms,
                spec.updated_at_epoch_ms
            ],
        )?;

        Ok(spec)
    }

    pub fn delete_alarm(&self, alarm_id: &str, now_epoch_ms: i64) -> Result<()> {
        let existing = self.get_alarm(alarm_id)?;
        Self::verify_n3_cooldown(&existing, now_epoch_ms)?;

        let affected = self.conn.execute(
            "UPDATE alarms
             SET enabled = 0,
                 cooldown_unlock_at_epoch_ms = NULL,
                 deleted_at_epoch_ms = ?2,
                 updated_at_epoch_ms = ?2
             WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
            params![alarm_id, now_epoch_ms],
        )?;

        if affected == 0 {
            return Err(CoreError::NotFound(alarm_id.to_string()));
        }
        Ok(())
    }

    pub fn reschedule_after_trigger(
        &self,
        alarm_id: &str,
        now_epoch_ms: i64,
    ) -> Result<Option<i64>> {
        let mut spec = self.get_alarm(alarm_id)?;
        if !spec.enabled {
            return Ok(None);
        }

        if spec.weekdays_mask == 0 {
            if spec.trigger_at_epoch_ms > now_epoch_ms {
                return Ok(Some(spec.trigger_at_epoch_ms));
            }
            spec.enabled = false;
            spec.updated_at_epoch_ms = now_epoch_ms;
            self.conn.execute(
                "UPDATE alarms
                 SET enabled = 0, updated_at_epoch_ms = ?2
                 WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
                params![alarm_id, now_epoch_ms],
            )?;
            return Ok(None);
        }

        let next_ms = compute_next_trigger_ms(
            spec.hour,
            spec.minute,
            spec.weekdays_mask,
            now_epoch_ms,
        )?;
        spec.trigger_at_epoch_ms = next_ms;
        spec.updated_at_epoch_ms = now_epoch_ms;

        self.conn.execute(
            "UPDATE alarms
             SET trigger_at_epoch_ms = ?2, updated_at_epoch_ms = ?3
             WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
            params![alarm_id, next_ms, now_epoch_ms],
        )?;

        Ok(Some(next_ms))
    }

    pub fn record_outcome(
        &self,
        alarm_id: &str,
        outcome: &str,
        recorded_at_epoch_ms: i64,
    ) -> Result<()> {
        self.record_outcome_with_proof(alarm_id, outcome, recorded_at_epoch_ms, None, None, None)
    }

    pub fn record_outcome_with_proof(
        &self,
        alarm_id: &str,
        outcome: &str,
        recorded_at_epoch_ms: i64,
        proof_image_path: Option<&str>,
        matched_label: Option<&str>,
        confidence: Option<f32>,
    ) -> Result<()> {
        match outcome {
            "CUMPRIDO" | "FALHOU" | "IGNORADO" | "ADIADO" => {}
            other => return Err(CoreError::InvalidOutcome(other.to_string())),
        }

        if let Some(conf) = confidence {
            if !conf.is_finite() || !(0.0..=1.0).contains(&conf) {
                return Err(CoreError::Validation(format!(
                    "Confiança inválida: {conf} (esperado 0.0..=1.0)"
                )));
            }
        }

        self.conn.execute(
            "INSERT INTO alarm_outcomes (
                alarm_id, outcome, recorded_at_epoch_ms,
                proof_image_path, matched_label, confidence
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                alarm_id,
                outcome,
                recorded_at_epoch_ms,
                proof_image_path,
                matched_label,
                confidence.map(f64::from)
            ],
        )?;
        Ok(())
    }

    pub fn list_outcomes(&self, alarm_id: &str) -> Result<Vec<AlarmOutcomeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                id, alarm_id, outcome, recorded_at_epoch_ms,
                proof_image_path, matched_label, confidence
             FROM alarm_outcomes
             WHERE alarm_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![alarm_id], |row| {
            let conf_f64: Option<f64> = row.get(6)?;
            Ok(AlarmOutcomeRecord {
                id: row.get(0)?,
                alarm_id: row.get(1)?,
                outcome: row.get(2)?,
                recorded_at_epoch_ms: row.get(3)?,
                proof_image_path: row.get(4)?,
                matched_label: row.get(5)?,
                confidence: conf_f64.map(|v| v as f32),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn create_note(&self, body: &str, now_epoch_ms: i64) -> Result<QuickNoteRecord> {
        let trimmed = body.trim();
        if trimmed.is_empty() {
            return Err(CoreError::Validation(
                "O texto da nota não pode estar vazio".to_string(),
            ));
        }

        let id = Uuid::now_v7().to_string();
        let tags = extract_hashtags(trimmed);
        let tags_csv = tags.join(",");

        self.conn.execute(
            "INSERT INTO quick_notes (
                id, body, tags_csv, created_at_epoch_ms, updated_at_epoch_ms, deleted_at_epoch_ms
             )
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
            params![id, trimmed, tags_csv, now_epoch_ms, now_epoch_ms],
        )?;

        Ok(QuickNoteRecord {
            id,
            body: trimmed.to_string(),
            tags,
            created_at_epoch_ms: now_epoch_ms,
            updated_at_epoch_ms: now_epoch_ms,
            deleted_at_epoch_ms: None,
        })
    }

    pub fn delete_note(&self, note_id: &str, now_epoch_ms: i64) -> Result<()> {
        let updated = self.conn.execute(
            "UPDATE quick_notes
             SET deleted_at_epoch_ms = ?2, updated_at_epoch_ms = ?2
             WHERE id = ?1 AND deleted_at_epoch_ms IS NULL",
            params![note_id, now_epoch_ms],
        )?;
        if updated == 0 {
            return Err(CoreError::NotFound(note_id.to_string()));
        }
        Ok(())
    }

    pub fn list_notes_for_range(
        &self,
        start_epoch_ms: i64,
        end_epoch_ms: i64,
        tag_filter: Option<&str>,
    ) -> Result<Vec<QuickNoteRecord>> {
        let normalized_filter = tag_filter
            .map(|t| t.trim().trim_start_matches('#').to_lowercase())
            .filter(|t| !t.is_empty());

        let mut stmt = self.conn.prepare(
            "SELECT id, body, tags_csv, created_at_epoch_ms, updated_at_epoch_ms, deleted_at_epoch_ms
             FROM quick_notes
             WHERE deleted_at_epoch_ms IS NULL
               AND created_at_epoch_ms >= ?1
               AND created_at_epoch_ms <= ?2
             ORDER BY created_at_epoch_ms DESC, id DESC",
        )?;

        let rows = stmt.query_map(params![start_epoch_ms, end_epoch_ms], |row| {
            let tags_csv: String = row.get(2)?;
            let tags: Vec<String> = tags_csv
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            Ok(QuickNoteRecord {
                id: row.get(0)?,
                body: row.get(1)?,
                tags,
                created_at_epoch_ms: row.get(3)?,
                updated_at_epoch_ms: row.get(4)?,
                deleted_at_epoch_ms: row.get(5)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            let record = row?;
            if let Some(ref wanted_tag) = normalized_filter {
                if !record.tags.iter().any(|t| t == wanted_tag) {
                    continue;
                }
            }
            out.push(record);
        }
        Ok(out)
    }

    pub fn list_outcomes_for_range(
        &self,
        start_epoch_ms: i64,
        end_epoch_ms: i64,
    ) -> Result<Vec<TimelineOutcomeEntry>> {
        let mut stmt = self.conn.prepare(
            "SELECT
                o.id,
                o.alarm_id,
                COALESCE(a.title, o.alarm_id) AS alarm_title,
                COALESCE(a.level, 3) AS alarm_level,
                o.outcome,
                o.recorded_at_epoch_ms,
                o.proof_image_path,
                o.matched_label,
                o.confidence
             FROM alarm_outcomes o
             LEFT JOIN alarms a ON a.id = o.alarm_id
             WHERE o.recorded_at_epoch_ms >= ?1
               AND o.recorded_at_epoch_ms <= ?2
             ORDER BY o.recorded_at_epoch_ms DESC, o.id DESC",
        )?;

        let rows = stmt.query_map(params![start_epoch_ms, end_epoch_ms], |row| {
            let level_raw: i64 = row.get(3)?;
            let conf_f64: Option<f64> = row.get(8)?;
            Ok(TimelineOutcomeEntry {
                id: row.get(0)?,
                alarm_id: row.get(1)?,
                alarm_title: row.get(2)?,
                alarm_level: AlarmLevel::from_i64(level_raw),
                outcome: row.get(4)?,
                recorded_at_epoch_ms: row.get(5)?,
                proof_image_path: row.get(6)?,
                matched_label: row.get(7)?,
                confidence: conf_f64.map(|v| v as f32),
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn daily_timeline(
        &self,
        reference_epoch_ms: i64,
        tag_filter: Option<&str>,
    ) -> Result<DailyTimelineSnapshot> {
        let ref_dt = match Local.timestamp_millis_opt(reference_epoch_ms) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt,
            LocalResult::None => {
                return Err(CoreError::Validation(format!(
                    "Timestamp de referência inválido: {reference_epoch_ms}"
                )))
            }
        };

        let date_naive = ref_dt.date_naive();
        let date_iso = date_naive.format("%Y-%m-%d").to_string();

        let start_time = NaiveTime::from_hms_milli_opt(0, 0, 0, 0)
            .ok_or_else(|| CoreError::Validation("Erro ao criar início do dia".to_string()))?;
        let end_time = NaiveTime::from_hms_milli_opt(23, 59, 59, 999)
            .ok_or_else(|| CoreError::Validation("Erro ao criar fim do dia".to_string()))?;

        let start_naive = date_naive.and_time(start_time);
        let end_naive = date_naive.and_time(end_time);

        let day_start_epoch_ms = match Local.from_local_datetime(&start_naive) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.timestamp_millis(),
            LocalResult::None => reference_epoch_ms,
        };
        let day_end_epoch_ms = match Local.from_local_datetime(&end_naive) {
            LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.timestamp_millis(),
            LocalResult::None => reference_epoch_ms + 86_399_999,
        };

        let weekday_bit = 1u8 << date_naive.weekday().num_days_from_monday();
        let all_alarms = self.list_alarms()?;
        let mut scheduled_alarms: Vec<AlarmSpec> = all_alarms
            .into_iter()
            .filter(|alarm| {
                if !alarm.enabled || alarm.created_at_epoch_ms > day_end_epoch_ms {
                    return false;
                }
                if alarm.weekdays_mask == 0 {
                    alarm.trigger_at_epoch_ms >= day_start_epoch_ms
                        && alarm.trigger_at_epoch_ms <= day_end_epoch_ms
                } else {
                    (alarm.weekdays_mask & weekday_bit) != 0
                }
            })
            .collect();
        scheduled_alarms.sort_by_key(|a| (a.hour, a.minute, a.id.clone()));

        let outcomes = self.list_outcomes_for_range(day_start_epoch_ms, day_end_epoch_ms)?;
        let notes = self.list_notes_for_range(day_start_epoch_ms, day_end_epoch_ms, tag_filter)?;

        Ok(DailyTimelineSnapshot {
            date_iso,
            day_start_epoch_ms,
            day_end_epoch_ms,
            scheduled_alarms,
            outcomes,
            notes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_alarm(id: &str, level: AlarmLevel, weekdays_mask: u8) -> AlarmSpec {
        AlarmSpec {
            id: id.to_string(),
            title: "Ginásio às 06:00".to_string(),
            level,
            target_label: if level == AlarmLevel::ProofOfAction {
                "Shoe".to_string()
            } else {
                String::new()
            },
            hour: 6,
            minute: 0,
            weekdays_mask,
            grace_window_minutes: 30,
            enabled: true,
            timezone: "Local".to_string(),
            trigger_at_epoch_ms: 1_700_000_000_000,
            cooldown_unlock_at_epoch_ms: None,
            created_at_epoch_ms: 1_700_000_000_000,
            updated_at_epoch_ms: 1_700_000_000_000,
            deleted_at_epoch_ms: None,
        }
    }

    #[test]
    fn roundtrip_alarm_crud_and_outcomes() -> Result<()> {
        let store = CoreStore::open_in_memory()?;
        let spec = sample_alarm("alarm-01", AlarmLevel::Persistent, 0b0001_1111);

        store.upsert_alarm(&spec)?;
        let loaded = store.get_alarm("alarm-01")?;
        assert_eq!(loaded, spec);

        let all = store.list_alarms()?;
        assert_eq!(all.len(), 1);

        let toggled = store.set_alarm_enabled("alarm-01", false, 1_700_000_001_000)?;
        assert!(!toggled.enabled);

        store.record_outcome("alarm-01", "CUMPRIDO", 1_700_000_005_000)?;
        store.record_outcome("alarm-01", "ADIADO", 1_700_000_006_000)?;
        let outcomes = store.list_outcomes("alarm-01")?;
        assert_eq!(outcomes.len(), 2);

        store.delete_alarm("alarm-01", 1_700_000_010_000)?;
        assert!(store.list_alarms()?.is_empty());
        assert!(matches!(
            store.get_alarm("alarm-01"),
            Err(CoreError::NotFound(_))
        ));

        Ok(())
    }

    #[test]
    fn n3_asymmetric_cooldown_enforced() -> Result<()> {
        let store = CoreStore::open_in_memory()?;
        let now = 1_700_000_000_000;

        // Upgrading N1 -> N3 is immediate
        let mut spec = sample_alarm("alarm-n3", AlarmLevel::Passive, 0);
        store.upsert_alarm(&spec)?;

        spec.level = AlarmLevel::ProofOfAction;
        spec.target_label = "Book".to_string();
        spec.updated_at_epoch_ms = now + 1_000;
        store.upsert_alarm(&spec)?;

        // Attempting to downgrade N3 -> N1 without cooldown fails
        let mut downgrade = spec.clone();
        downgrade.level = AlarmLevel::Passive;
        downgrade.updated_at_epoch_ms = now + 2_000;
        assert!(matches!(
            store.upsert_alarm(&downgrade),
            Err(CoreError::CooldownActive { .. })
        ));

        // Attempting to delete N3 without cooldown fails
        assert!(matches!(
            store.delete_alarm("alarm-n3", now + 2_000),
            Err(CoreError::CooldownActive { .. })
        ));

        // Request cooldown unlock and wait 24h
        let unlock_at = store.request_cooldown_unlock("alarm-n3", now + 3_000)?;
        assert_eq!(unlock_at, now + 3_000 + COOLDOWN_DURATION_MS);

        // Before 24h still blocked
        assert!(matches!(
            store.delete_alarm("alarm-n3", unlock_at - 1),
            Err(CoreError::CooldownActive { .. })
        ));

        // At/after 24h allowed
        store.delete_alarm("alarm-n3", unlock_at)?;
        assert!(store.list_alarms()?.is_empty());

        Ok(())
    }

    #[test]
    fn validation_and_reschedule_behavior() -> Result<()> {
        let store = CoreStore::open_in_memory()?;
        let now = 1_700_000_000_000;

        // Invalid weekday mask (bit 7 set)
        assert!(matches!(
            compute_next_trigger_ms(6, 0, 0b1000_0000, now),
            Err(CoreError::Validation(_))
        ));

        // Missing target_label for N3
        let mut invalid_n3 = sample_alarm("bad-n3", AlarmLevel::ProofOfAction, 0);
        invalid_n3.target_label = String::new();
        assert!(matches!(
            store.upsert_alarm(&invalid_n3),
            Err(CoreError::Validation(_))
        ));

        // One-shot alarm with future trigger_at_epoch_ms preserves schedule on early test trigger
        let mut one_shot = sample_alarm("one-shot", AlarmLevel::Persistent, 0);
        one_shot.trigger_at_epoch_ms = now + 60_000;
        store.upsert_alarm(&one_shot)?;

        let preserved = store.reschedule_after_trigger("one-shot", now + 10_000)?;
        assert_eq!(preserved, Some(now + 60_000));

        // Once real trigger time has passed, one-shot disables itself
        let finished = store.reschedule_after_trigger("one-shot", now + 61_000)?;
        assert_eq!(finished, None);
        assert!(!store.get_alarm("one-shot")?.enabled);

        // Recurring alarm computes next trigger > now
        let recurring = sample_alarm("rec-01", AlarmLevel::Persistent, 0b0111_1111);
        store.upsert_alarm(&recurring)?;
        let next_ms = store.reschedule_after_trigger("rec-01", now)?;
        assert!(next_ms.is_some_and(|ms| ms > now));

        // Disabled recurring alarm does not reschedule
        store.set_alarm_enabled("rec-01", false, now + 1_000)?;
        let disabled_next = store.reschedule_after_trigger("rec-01", now + 2_000)?;
        assert_eq!(disabled_next, None);

        Ok(())
    }

    #[test]
    fn curated_synonyms_and_proof_outcomes() -> Result<()> {
        assert!(matches_curated_label("Shoe", "Footwear"));
        assert!(matches_curated_label("Shoe", "sneakers"));
        assert!(matches_curated_label("Book", "Publication"));
        assert!(matches_curated_label("Bottle", "Water bottle"));
        assert!(matches_curated_label("Cup", "Mug"));
        assert!(matches_curated_label("Laptop", "Personal computer"));
        assert!(!matches_curated_label("Shoe", "Mug"));
        assert!(!matches_curated_label("Book", ""));

        let store = CoreStore::open_in_memory()?;
        store.record_outcome_with_proof(
            "alarm-proof-1",
            "CUMPRIDO",
            1_700_000_010_000,
            Some("/data/user/0/com.crabcheck.app/files/proofs/proof_1.jpg"),
            Some("Sneakers"),
            Some(0.88),
        )?;

        let outcomes = store.list_outcomes("alarm-proof-1")?;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(
            outcomes[0].proof_image_path.as_deref(),
            Some("/data/user/0/com.crabcheck.app/files/proofs/proof_1.jpg")
        );
        assert_eq!(outcomes[0].matched_label.as_deref(), Some("Sneakers"));
        assert!(outcomes[0]
            .confidence
            .is_some_and(|c| (c - 0.88).abs() < 0.001));

        // Reject out-of-range or NaN confidence
        assert!(matches!(
            store.record_outcome_with_proof(
                "alarm-proof-1",
                "CUMPRIDO",
                1_700_000_020_000,
                None,
                Some("Shoe"),
                Some(1.5),
            ),
            Err(CoreError::Validation(_))
        ));

        Ok(())
    }

    #[test]
    fn quick_notes_tags_and_daily_timeline() -> Result<()> {
        let tags = extract_hashtags("Treino matinal concluído! #Foco #treino_pesado, #foco #energia-100.");
        assert_eq!(tags, vec!["foco", "treino_pesado", "energia-100"]);

        let store = CoreStore::open_in_memory()?;
        let now = 1_700_050_000_000;

        let mut alarm = sample_alarm("alarm-tl-1", AlarmLevel::ProofOfAction, 0b0111_1111);
        alarm.created_at_epoch_ms = now - 10_000;
        alarm.updated_at_epoch_ms = now - 10_000;
        store.upsert_alarm(&alarm)?;

        // Alarm created in the future should not appear in today's timeline (R4)
        let mut future_alarm = sample_alarm("alarm-future", AlarmLevel::Passive, 0b0111_1111);
        future_alarm.created_at_epoch_ms = now + 100_000_000;
        future_alarm.updated_at_epoch_ms = now + 100_000_000;
        store.upsert_alarm(&future_alarm)?;

        store.record_outcome_with_proof(
            "alarm-tl-1",
            "CUMPRIDO",
            now + 1_000,
            Some("/data/proof.jpg"),
            Some("Shoe"),
            Some(0.92),
        )?;

        let n1 = store.create_note("Leitura de 20 páginas #leitura #habito", now + 2_000)?;
        let n2 = store.create_note("Sessão de corrida #corrida #habito", now + 3_000)?;
        assert_eq!(n1.tags, vec!["leitura", "habito"]);

        let snap_all = store.daily_timeline(now, None)?;
        assert_eq!(snap_all.scheduled_alarms.len(), 1);
        assert_eq!(snap_all.scheduled_alarms[0].id, "alarm-tl-1");
        assert_eq!(snap_all.outcomes.len(), 1);
        assert_eq!(snap_all.outcomes[0].alarm_title, "Ginásio às 06:00");
        assert_eq!(snap_all.notes.len(), 2);

        let snap_filtered = store.daily_timeline(now, Some("#leitura"))?;
        assert_eq!(snap_filtered.notes.len(), 1);
        assert_eq!(snap_filtered.notes[0].id, n1.id);

        store.delete_note(&n2.id, now + 4_000)?;
        let snap_after_del = store.daily_timeline(now, None)?;
        assert_eq!(snap_after_del.notes.len(), 1);
        assert_eq!(snap_after_del.notes[0].id, n1.id);

        Ok(())
    }
}
