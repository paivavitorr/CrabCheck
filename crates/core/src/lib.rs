use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Invalid alarm outcome: {0}")]
    InvalidOutcome(String),
    #[error("Alarm not found: {0}")]
    NotFound(String),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmSpec {
    pub id: String,
    pub title: String,
    pub level: AlarmLevel,
    pub target_label: String,
    pub trigger_at_epoch_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlarmOutcomeRecord {
    pub id: i64,
    pub alarm_id: String,
    pub outcome: String,
    pub recorded_at_epoch_ms: i64,
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
                trigger_at_epoch_ms INTEGER NOT NULL
            );
            CREATE TABLE IF NOT EXISTS alarm_outcomes (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                alarm_id TEXT NOT NULL,
                outcome TEXT NOT NULL,
                recorded_at_epoch_ms INTEGER NOT NULL
            );",
        )?;
        Ok(())
    }

    pub fn upsert_alarm(&self, spec: &AlarmSpec) -> Result<()> {
        self.conn.execute(
            "INSERT INTO alarms (id, title, level, target_label, trigger_at_epoch_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                level = excluded.level,
                target_label = excluded.target_label,
                trigger_at_epoch_ms = excluded.trigger_at_epoch_ms",
            params![
                spec.id,
                spec.title,
                spec.level as i64,
                spec.target_label,
                spec.trigger_at_epoch_ms
            ],
        )?;
        Ok(())
    }

    pub fn get_alarm(&self, alarm_id: &str) -> Result<AlarmSpec> {
        let maybe_spec = self
            .conn
            .query_row(
                "SELECT id, title, level, target_label, trigger_at_epoch_ms
                 FROM alarms WHERE id = ?1",
                params![alarm_id],
                |row| {
                    let level_raw: i64 = row.get(2)?;
                    Ok(AlarmSpec {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        level: AlarmLevel::from_i64(level_raw),
                        target_label: row.get(3)?,
                        trigger_at_epoch_ms: row.get(4)?,
                    })
                },
            )
            .optional()?;

        maybe_spec.ok_or_else(|| CoreError::NotFound(alarm_id.to_string()))
    }

    pub fn record_outcome(
        &self,
        alarm_id: &str,
        outcome: &str,
        recorded_at_epoch_ms: i64,
    ) -> Result<()> {
        match outcome {
            "CUMPRIDO" | "FALHOU" | "IGNORADO" => {}
            other => return Err(CoreError::InvalidOutcome(other.to_string())),
        }

        self.conn.execute(
            "INSERT INTO alarm_outcomes (alarm_id, outcome, recorded_at_epoch_ms)
             VALUES (?1, ?2, ?3)",
            params![alarm_id, outcome, recorded_at_epoch_ms],
        )?;
        Ok(())
    }

    pub fn list_outcomes(&self, alarm_id: &str) -> Result<Vec<AlarmOutcomeRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, alarm_id, outcome, recorded_at_epoch_ms
             FROM alarm_outcomes
             WHERE alarm_id = ?1
             ORDER BY id ASC",
        )?;
        let rows = stmt.query_map(params![alarm_id], |row| {
            Ok(AlarmOutcomeRecord {
                id: row.get(0)?,
                alarm_id: row.get(1)?,
                outcome: row.get(2)?,
                recorded_at_epoch_ms: row.get(3)?,
            })
        })?;

        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_alarm_and_outcomes() -> Result<()> {
        let store = CoreStore::open_in_memory()?;
        let spec = AlarmSpec {
            id: "spike-001".to_string(),
            title: "Ginásio às 06:00".to_string(),
            level: AlarmLevel::ProofOfAction,
            target_label: "Shoe".to_string(),
            trigger_at_epoch_ms: 1_700_000_000_000,
        };

        store.upsert_alarm(&spec)?;
        let loaded = store.get_alarm("spike-001")?;
        assert_eq!(loaded, spec);

        store.record_outcome("spike-001", "CUMPRIDO", 1_700_000_005_000)?;
        let outcomes = store.list_outcomes("spike-001")?;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].outcome, "CUMPRIDO");

        let invalid = store.record_outcome("spike-001", "INVALIDO", 1_700_000_006_000);
        assert!(matches!(invalid, Err(CoreError::InvalidOutcome(_))));

        Ok(())
    }
}
