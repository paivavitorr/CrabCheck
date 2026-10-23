#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::OnceLock;

use crabcheck_core::{
    day_reference_epoch_ms, format_local_time_hm, format_local_time_hms, AlarmDraft, AlarmLevel,
    AlarmOutcomeRecord, CoreError, CoreStore, DailyTimelineSnapshot, CURATED_OBJECT_LABELS,
};
use dioxus::prelude::*;

// Ensure crabcheck-core-ffi symbols remain linked into the main binary on Android
#[used]
static KEEP_FFI: fn(
    String,
    String,
) -> Result<crabcheck_core_ffi::FfiAlarmDto, crabcheck_core_ffi::FfiError> =
    crabcheck_core_ffi::ffi_get_alarm;

static DB_PATH_CACHE: OnceLock<String> = OnceLock::new();

const CRABCHECK_LOGO_DATA_URI: &str = include_str!("../assets/crab_b64.txt");

const WEEKDAY_NAMES: &[(&str, u8)] = &[
    ("Seg", 1 << 0),
    ("Ter", 1 << 1),
    ("Qua", 1 << 2),
    ("Qui", 1 << 3),
    ("Sex", 1 << 4),
    ("Sáb", 1 << 5),
    ("Dom", 1 << 6),
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct CalendarWeekDay {
    offset: i64,
    day_num: u32,
    weekday_short: &'static str,
    is_today: bool,
    is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MonthGridDay {
    offset: i64,
    day_num: u32,
    is_today: bool,
    is_selected: bool,
}

fn calculate_week_strip(now_epoch_ms: i64, selected_offset: i64) -> (String, Vec<CalendarWeekDay>) {
    use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};

    let today = match Local.timestamp_millis_opt(now_epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.date_naive(),
        LocalResult::None => NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
    };

    let selected_date = if selected_offset >= 0 {
        today.checked_add_days(Days::new(selected_offset as u64)).unwrap_or(today)
    } else {
        today.checked_sub_days(Days::new((-selected_offset) as u64)).unwrap_or(today)
    };

    let weekday_idx = selected_date.weekday().num_days_from_monday();
    let monday = selected_date.checked_sub_days(Days::new(weekday_idx as u64)).unwrap_or(selected_date);

    let weekday_labels = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
    let mut days = Vec::with_capacity(7);

    for (i, label) in weekday_labels.iter().enumerate() {
        if let Some(d) = monday.checked_add_days(Days::new(i as u64)) {
            let offset = (d - today).num_days();
            days.push(CalendarWeekDay {
                offset,
                day_num: d.day(),
                weekday_short: label,
                is_today: d == today,
                is_selected: offset == selected_offset,
            });
        }
    }

    let month_name = match selected_date.month() {
        1 => "Janeiro", 2 => "Fevereiro", 3 => "Março", 4 => "Abril",
        5 => "Maio", 6 => "Junho", 7 => "Julho", 8 => "Agosto",
        9 => "Setembro", 10 => "Outubro", 11 => "Novembro", 12 => "Dezembro",
        _ => "",
    };
    let month_header = format!("{month_name} {}", selected_date.year());

    (month_header, days)
}

fn calculate_month_grid(now_epoch_ms: i64, selected_offset: i64) -> (String, Vec<Option<MonthGridDay>>) {
    use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};

    let today = match Local.timestamp_millis_opt(now_epoch_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.date_naive(),
        LocalResult::None => NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
    };

    let selected_date = if selected_offset >= 0 {
        today.checked_add_days(Days::new(selected_offset as u64)).unwrap_or(today)
    } else {
        today.checked_sub_days(Days::new((-selected_offset) as u64)).unwrap_or(today)
    };

    let y = selected_date.year();
    let m = selected_date.month();
    let first_day = NaiveDate::from_ymd_opt(y, m, 1).unwrap_or(selected_date);
    let start_col = first_day.weekday().num_days_from_monday() as usize;

    let days_in_month = if m == 12 {
        NaiveDate::from_ymd_opt(y + 1, 1, 1).unwrap_or(first_day) - first_day
    } else {
        NaiveDate::from_ymd_opt(y, m + 1, 1).unwrap_or(first_day) - first_day
    }.num_days() as u32;

    let mut grid = Vec::new();
    for _ in 0..start_col {
        grid.push(None);
    }
    for d in 1..=days_in_month {
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            let offset = (date - today).num_days();
            grid.push(Some(MonthGridDay {
                offset,
                day_num: d,
                is_today: date == today,
                is_selected: offset == selected_offset,
            }));
        }
    }

    let month_name = match m {
        1 => "Janeiro", 2 => "Fevereiro", 3 => "Março", 4 => "Abril",
        5 => "Maio", 6 => "Junho", 7 => "Julho", 8 => "Agosto",
        9 => "Setembro", 10 => "Outubro", 11 => "Novembro", 12 => "Dezembro",
        _ => "",
    };
    let month_header = format!("{month_name} {y}");

    (month_header, grid)
}

fn prev_month_offset(now_ms: i64, current_offset: i64) -> i64 {
    use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};
    let today = match Local.timestamp_millis_opt(now_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.date_naive(),
        LocalResult::None => NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
    };
    let current_date = if current_offset >= 0 {
        today.checked_add_days(Days::new(current_offset as u64)).unwrap_or(today)
    } else {
        today.checked_sub_days(Days::new((-current_offset) as u64)).unwrap_or(today)
    };
    let (prev_y, prev_m) = if current_date.month() == 1 {
        (current_date.year() - 1, 12)
    } else {
        (current_date.year(), current_date.month() - 1)
    };
    let target = NaiveDate::from_ymd_opt(prev_y, prev_m, 1).unwrap_or(current_date);
    (target - today).num_days()
}

fn next_month_offset(now_ms: i64, current_offset: i64) -> i64 {
    use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};
    let today = match Local.timestamp_millis_opt(now_ms) {
        LocalResult::Single(dt) | LocalResult::Ambiguous(dt, _) => dt.date_naive(),
        LocalResult::None => NaiveDate::from_ymd_opt(2026, 10, 23).unwrap(),
    };
    let current_date = if current_offset >= 0 {
        today.checked_add_days(Days::new(current_offset as u64)).unwrap_or(today)
    } else {
        today.checked_sub_days(Days::new((-current_offset) as u64)).unwrap_or(today)
    };
    let (next_y, next_m) = if current_date.month() == 12 {
        (current_date.year() + 1, 1)
    } else {
        (current_date.year(), current_date.month() + 1)
    };
    let target = NaiveDate::from_ymd_opt(next_y, next_m, 1).unwrap_or(current_date);
    (target - today).num_days()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Timeline,
    Habits,
    Health,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PermissionHealthState {
    exact_alarm: bool,
    overlay: bool,
    accessibility: bool,
    camera: bool,
    notifications: bool,
    battery: bool,
}

impl PermissionHealthState {
    fn all_granted_default() -> Self {
        Self {
            exact_alarm: true,
            overlay: true,
            accessibility: true,
            camera: true,
            notifications: true,
            battery: true,
        }
    }

    fn from_csv(csv: &str) -> Self {
        let mut state = Self {
            exact_alarm: false,
            overlay: false,
            accessibility: false,
            camera: false,
            notifications: false,
            battery: false,
        };
        for part in csv.split(',') {
            if let Some((k, v)) = part.split_once('=') {
                let granted = v.trim() == "1";
                match k.trim() {
                    "exact_alarm" => state.exact_alarm = granted,
                    "overlay" => state.overlay = granted,
                    "accessibility" => state.accessibility = granted,
                    "camera" => state.camera = granted,
                    "notifications" => state.notifications = granted,
                    "battery" => state.battery = granted,
                    _ => {}
                }
            }
        }
        state
    }

    fn all_granted(&self) -> bool {
        self.exact_alarm
            && self.overlay
            && self.accessibility
            && self.camera
            && self.notifications
            && self.battery
    }

    fn granted_count(&self) -> usize {
        [
            self.exact_alarm,
            self.overlay,
            self.accessibility,
            self.camera,
            self.notifications,
            self.battery,
        ]
        .into_iter()
        .filter(|b| *b)
        .count()
    }

    fn missing_labels(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        if !self.exact_alarm {
            missing.push("Alarme Exato");
        }
        if !self.overlay {
            missing.push("Sobreposição (Overlay)");
        }
        if !self.accessibility {
            missing.push("Acessibilidade N3");
        }
        if !self.camera {
            missing.push("Câmara");
        }
        if !self.notifications {
            missing.push("Notificações / Full-Screen");
        }
        if !self.battery {
            missing.push("Isenção de Bateria");
        }
        missing
    }
}

fn empty_timeline_snapshot() -> DailyTimelineSnapshot {
    DailyTimelineSnapshot {
        date_iso: String::from("Hoje"),
        day_start_epoch_ms: 0,
        day_end_epoch_ms: 0,
        scheduled_alarms: Vec::new(),
        outcomes: Vec::new(),
        notes: Vec::new(),
    }
}

fn main() {
    #[cfg(target_os = "android")]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut active_tab = use_signal(|| ActiveTab::Timeline);
    let mut health_state = use_signal(check_native_permissions);
    let mut permission_blocked_n3 = use_signal(|| false);

    // Timeline signals
    let mut selected_day_offset = use_signal(|| 0i64);
    let mut show_month_picker = use_signal(|| false);
    let mut active_tag_filter = use_signal(|| Option::<String>::None);
    let mut note_draft = use_signal(String::new);
    let mut timeline_snapshot = use_signal(|| {
        let db = resolve_db_path();
        let ref_ms = day_reference_epoch_ms(current_epoch_ms(), 0);
        CoreStore::open(db)
            .and_then(|s| s.daily_timeline(ref_ms, None))
            .unwrap_or_else(|_| empty_timeline_snapshot())
    });

    // Habits signals
    let mut alarms = use_signal(|| {
        let db = resolve_db_path();
        CoreStore::open(db)
            .and_then(|s| s.list_alarms())
            .unwrap_or_default()
    });
    let mut selected_alarm_id = use_signal(|| Option::<String>::None);
    let mut outcomes = use_signal(Vec::<AlarmOutcomeRecord>::new);
    let mut status_msg = use_signal(String::new);
    let mut cooldown_blocked_id = use_signal(|| Option::<String>::None);

    // Form signals
    let mut editing_id = use_signal(|| Option::<String>::None);
    let mut title = use_signal(|| String::from("Ginásio às 06:00"));
    let mut hour = use_signal(|| 6u8);
    let mut minute = use_signal(|| 0u8);
    let mut weekdays_mask = use_signal(|| 0b0001_1111u8);
    let mut level = use_signal(|| AlarmLevel::ProofOfAction);
    let mut target_label = use_signal(|| String::from("Shoe"));
    let mut grace_window_minutes = use_signal(|| 30u16);
    let mut enabled = use_signal(|| true);

    let mut refresh_timeline = move |day_offset: i64, tag_filter: Option<String>| {
        let db = resolve_db_path();
        let ref_ms = day_reference_epoch_ms(current_epoch_ms(), day_offset);
        if let Ok(store) = CoreStore::open(db) {
            if let Ok(snap) = store.daily_timeline(ref_ms, tag_filter.as_deref()) {
                timeline_snapshot.set(snap);
            }
        }
    };

    let mut reload_all = move |maybe_select_id: Option<String>| {
        let db = resolve_db_path();
        let current_health = check_native_permissions();
        if current_health.all_granted() {
            permission_blocked_n3.set(false);
        }
        health_state.set(current_health);

        match CoreStore::open(db) {
            Ok(store) => {
                if let Ok(list) = store.list_alarms() {
                    alarms.set(list);
                }
                let target_id = maybe_select_id.or_else(|| selected_alarm_id.read().clone());
                if let Some(id) = target_id {
                    if let Ok(outs) = store.list_outcomes(&id) {
                        outcomes.set(outs);
                    }
                    selected_alarm_id.set(Some(id));
                }
                let ref_ms = day_reference_epoch_ms(current_epoch_ms(), selected_day_offset());
                let filter = active_tag_filter.read().clone();
                if let Ok(snap) = store.daily_timeline(ref_ms, filter.as_deref()) {
                    timeline_snapshot.set(snap);
                }
            }
            Err(err) => status_msg.set(format!("Erro ao abrir SQLite: {err}")),
        }
    };

    let mut reset_form = move || {
        editing_id.set(None);
        title.set(String::new());
        hour.set(6);
        minute.set(0);
        weekdays_mask.set(0b0001_1111);
        level.set(AlarmLevel::ProofOfAction);
        target_label.set(String::from("Shoe"));
        grace_window_minutes.set(30);
        enabled.set(true);
    };

    let save_alarm = move |_| {
        let current_health = check_native_permissions();
        health_state.set(current_health.clone());

        // R1: Block only when resulting state is Level 3 with enabled == true
        if level() == AlarmLevel::ProofOfAction && enabled() && !current_health.all_granted() {
            permission_blocked_n3.set(true);
            let missing = current_health.missing_labels().join(", ");
            status_msg.set(format!(
                "Permissões obrigatórias pendentes para Nível 3: {missing}."
            ));
            return;
        }
        permission_blocked_n3.set(false);

        let db = resolve_db_path();
        let now_ms = current_epoch_ms();
        let store = match CoreStore::open(db) {
            Ok(s) => s,
            Err(err) => {
                status_msg.set(format!("Erro ao abrir SQLite: {err}"));
                return;
            }
        };

        let existing = editing_id()
            .as_deref()
            .and_then(|id| store.get_alarm(id).ok());

        let draft = AlarmDraft {
            id: editing_id(),
            title: title(),
            level: level(),
            target_label: if level() == AlarmLevel::ProofOfAction {
                target_label()
            } else {
                String::new()
            },
            hour: hour(),
            minute: minute(),
            weekdays_mask: weekdays_mask(),
            grace_window_minutes: grace_window_minutes(),
            enabled: enabled(),
            timezone: String::from("Local"),
        };

        let spec = match crabcheck_core::AlarmSpec::new_from_draft(draft, now_ms, existing.as_ref())
        {
            Ok(s) => s,
            Err(err) => {
                status_msg.set(format!("Validação: {err}"));
                return;
            }
        };

        match store.upsert_alarm(&spec) {
            Ok(()) => {
                cooldown_blocked_id.set(None);
                let jni_res = if spec.enabled {
                    schedule_native_alarm(&spec.id, spec.trigger_at_epoch_ms)
                } else {
                    cancel_native_alarm(&spec.id)
                };

                match jni_res {
                    Ok(()) => {
                        status_msg.set(format!(
                            "Hábito '{}' ({:02}:{:02}) guardado com sucesso.",
                            spec.title, spec.hour, spec.minute
                        ));
                    }
                    Err(err) => {
                        status_msg.set(format!("Guardado no SQLite, aviso JNI: {err}"));
                    }
                }
                let saved_id = spec.id.clone();
                reset_form();
                reload_all(Some(saved_id));
            }
            Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                cooldown_blocked_id.set(Some(spec.id.clone()));
                status_msg.set(format!(
                    "Cooldown N3 ativo! Alterações de alívio só após {}",
                    unlock_at_epoch_ms
                ));
            }
            Err(err) => {
                status_msg.set(format!("Erro ao guardar hábito: {err}"));
            }
        }
    };

    let save_quick_note = move |_| {
        let text = note_draft();
        if text.trim().is_empty() {
            status_msg.set(String::from("Escreva algum conteúdo antes de guardar a nota."));
            return;
        }
        let db = resolve_db_path();
        let now_ms = current_epoch_ms();
        // R3: Anchor note timestamp to the viewed day when selected_day_offset != 0
        let note_ts = day_reference_epoch_ms(now_ms, selected_day_offset());
        match CoreStore::open(db).and_then(|s| s.create_note(&text, note_ts)) {
            Ok(created) => {
                note_draft.set(String::new());
                let tags_info = if created.tags.is_empty() {
                    String::new()
                } else {
                    format!(" (#{})", created.tags.join(" #"))
                };
                status_msg.set(format!("Nota rápida registada na timeline{tags_info}."));
                refresh_timeline(selected_day_offset(), active_tag_filter.read().clone());
            }
            Err(err) => status_msg.set(format!("Erro ao guardar nota: {err}")),
        }
    };

    let snap = timeline_snapshot();
    let current_health = health_state();
    let granted_count = current_health.granted_count();
    let now_epoch = current_epoch_ms();
    let (week_month_title, week_days) = calculate_week_strip(now_epoch, selected_day_offset());
    let (month_title, month_cells) = calculate_month_grid(now_epoch, selected_day_offset());

    const GLOBAL_CSS: &str = r#"
        * { box-sizing: border-box; -webkit-tap-highlight-color: transparent; }
        html, body {
            margin: 0;
            padding: 0;
            background: #F6F7F9;
            color: #101828;
            font-family: -apple-system, BlinkMacSystemFont, "Inter", "Segoe UI", Roboto, sans-serif;
            -webkit-font-smoothing: antialiased;
        }
        .app-shell {
            min-height: 100vh;
            max-width: 520px;
            margin: 0 auto;
            padding: 0 0 84px 0;
            background: #F6F7F9;
        }
        .topbar {
            position: sticky;
            top: 0;
            z-index: 20;
            background: rgba(255, 255, 255, 0.94);
            backdrop-filter: blur(10px);
            border-bottom: 1px solid #E4E7EC;
            padding: 13px 16px;
            display: flex;
            justify-content: space-between;
            align-items: center;
        }
        .brand-row {
            display: flex;
            align-items: center;
            gap: 9px;
        }
        .brand-logo-img {
            width: 26px;
            height: 26px;
            border-radius: 6px;
            object-fit: contain;
        }
        .brand-mark {
            width: 24px;
            height: 24px;
            border-radius: 6px;
            background: #101828;
            display: flex;
            align-items: center;
            justify-content: center;
        }
        .brand-name {
            margin: 0;
            font-size: 16px;
            font-weight: 600;
            letter-spacing: -0.02em;
            color: #101828;
        }
        .health-pill {
            display: inline-flex;
            align-items: center;
            gap: 6px;
            padding: 4px 10px;
            border-radius: 999px;
            font-size: 12px;
            font-weight: 500;
            background: #F2F4F7;
            color: #344054;
            border: 1px solid #E4E7EC;
            cursor: pointer;
        }
        .dot-ok {
            width: 6px;
            height: 6px;
            border-radius: 50%;
            background: #12B76A;
        }
        .dot-warn {
            width: 6px;
            height: 6px;
            border-radius: 50%;
            background: #F79009;
        }
        .page-body {
            padding: 16px;
        }
        .surface-group {
            background: #FFFFFF;
            border: 1px solid #E4E7EC;
            border-radius: 12px;
            overflow: hidden;
            margin-bottom: 16px;
        }
        .surface-pad {
            background: #FFFFFF;
            border: 1px solid #E4E7EC;
            border-radius: 12px;
            padding: 14px;
            margin-bottom: 16px;
        }
        .list-row {
            padding: 13px 14px;
            border-bottom: 1px solid #F2F4F7;
            display: flex;
            justify-content: space-between;
            align-items: center;
            gap: 12px;
        }
        .list-row:last-child {
            border-bottom: none;
        }
        .section-head {
            display: flex;
            justify-content: space-between;
            align-items: baseline;
            margin: 20px 2px 8px 2px;
        }
        .section-head:first-child {
            margin-top: 4px;
        }
        .section-title {
            font-size: 13px;
            font-weight: 600;
            color: #475467;
            margin: 0;
        }
        .section-meta {
            font-size: 12px;
            color: #667085;
        }
        .time-display {
            font-variant-numeric: tabular-nums;
            font-size: 25px;
            font-weight: 600;
            letter-spacing: -0.03em;
            color: #101828;
            line-height: 1;
            min-width: 66px;
        }
        .field-input {
            width: 100%;
            padding: 9px 11px;
            background: #FFFFFF;
            border: 1px solid #D0D5DD;
            border-radius: 8px;
            font-size: 14px;
            color: #101828;
            outline: none;
        }
        .btn-primary {
            padding: 9px 14px;
            background: #101828;
            color: #FFFFFF;
            border: 1px solid #101828;
            border-radius: 8px;
            font-size: 13px;
            font-weight: 500;
        }
        .btn-secondary {
            padding: 6px 11px;
            background: #FFFFFF;
            color: #344054;
            border: 1px solid #D0D5DD;
            border-radius: 7px;
            font-size: 12px;
            font-weight: 500;
        }
        .btn-danger-ghost {
            padding: 6px 11px;
            background: #FFFFFF;
            color: #B42318;
            border: 1px solid #FDA29B;
            border-radius: 7px;
            font-size: 12px;
            font-weight: 500;
        }
        .bottom-nav {
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            height: 62px;
            background: rgba(255, 255, 255, 0.96);
            backdrop-filter: blur(10px);
            border-top: 1px solid #E4E7EC;
            display: flex;
            justify-content: space-around;
            align-items: center;
            z-index: 30;
            max-width: 520px;
            margin: 0 auto;
        }
        .nav-item {
            flex: 1;
            height: 100%;
            background: transparent;
            border: none;
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            gap: 4px;
            font-size: 11px;
            font-weight: 500;
            color: #667085;
        }
        .nav-item-active {
            color: #101828;
            font-weight: 600;
        }
        .cal-card {
            background: #FFFFFF;
            border: 1px solid #E4E7EC;
            border-radius: 12px;
            padding: 12px;
            margin-bottom: 14px;
        }
        .cal-top-row {
            display: flex;
            justify-content: space-between;
            align-items: center;
            margin-bottom: 10px;
        }
        .cal-title-btn {
            background: transparent;
            border: none;
            padding: 4px 6px;
            font-size: 14px;
            font-weight: 600;
            color: #101828;
            display: inline-flex;
            align-items: center;
            gap: 6px;
            cursor: pointer;
            border-radius: 6px;
        }
        .cal-nav-group {
            display: flex;
            align-items: center;
            gap: 6px;
        }
        .cal-btn-today {
            padding: 4px 9px;
            background: #F2F4F7;
            color: #344054;
            border: 1px solid #D0D5DD;
            border-radius: 6px;
            font-size: 11px;
            font-weight: 600;
            cursor: pointer;
        }
        .cal-nav-arrow {
            width: 28px;
            height: 28px;
            display: inline-flex;
            align-items: center;
            justify-content: center;
            background: #FFFFFF;
            border: 1px solid #D0D5DD;
            border-radius: 6px;
            color: #344054;
            font-size: 11px;
            cursor: pointer;
        }
        .week-strip-grid {
            display: grid;
            grid-template-columns: repeat(7, 1fr);
            gap: 4px;
        }
        .cal-day-cell {
            display: flex;
            flex-direction: column;
            align-items: center;
            justify-content: center;
            padding: 6px 2px;
            border-radius: 8px;
            background: transparent;
            border: 1px solid transparent;
            cursor: pointer;
        }
        .cal-day-weekday {
            font-size: 10px;
            font-weight: 500;
            color: #667085;
            text-transform: uppercase;
            letter-spacing: 0.02em;
            margin-bottom: 3px;
        }
        .cal-day-num {
            font-size: 14px;
            font-weight: 600;
            font-variant-numeric: tabular-nums;
            color: #101828;
            width: 26px;
            height: 26px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: 50%;
        }
        .cal-day-cell.is-selected .cal-day-num {
            background: #101828;
            color: #FFFFFF;
        }
        .cal-day-cell.is-selected .cal-day-weekday {
            color: #101828;
            font-weight: 600;
        }
        .cal-day-cell.is-today:not(.is-selected) .cal-day-num {
            border: 1.5px solid #101828;
            color: #101828;
        }
        .cal-month-table {
            display: grid;
            grid-template-columns: repeat(7, 1fr);
            gap: 4px;
            margin-top: 6px;
        }
        .cal-weekday-header {
            font-size: 10px;
            font-weight: 600;
            color: #98A2B3;
            text-align: center;
            padding-bottom: 4px;
            text-transform: uppercase;
        }
        .cal-month-cell {
            aspect-ratio: 1;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: 8px;
            background: transparent;
            border: 1px solid transparent;
            font-size: 13px;
            font-weight: 500;
            font-variant-numeric: tabular-nums;
            color: #101828;
            cursor: pointer;
        }
        .cal-month-cell.is-selected {
            background: #101828;
            color: #FFFFFF;
            font-weight: 600;
        }
        .cal-month-cell.is-today:not(.is-selected) {
            border: 1.5px solid #101828;
            font-weight: 600;
        }
        .cal-month-cell.is-empty {
            cursor: default;
        }
    "#;

    rsx! {
        div {
            class: "app-shell",
            style { "{GLOBAL_CSS}" }

            // Sticky Minimal Top Bar
            header {
                class: "topbar",
                div {
                    class: "brand-row",
                    img {
                        class: "brand-logo-img",
                        src: "{CRABCHECK_LOGO_DATA_URI}",
                        alt: "CrabCheck",
                    }
                    h1 { class: "brand-name", "CrabCheck" }
                }
                button {
                    class: "health-pill",
                    onclick: move |_| {
                        health_state.set(check_native_permissions());
                        active_tab.set(ActiveTab::Health);
                    },
                    span {
                        class: if current_health.all_granted() { "dot-ok" } else { "dot-warn" }
                    }
                    "{granted_count}/6 permissões"
                }
            }

            div {
                class: "page-body",

                // Subtle dismissible toast only when status_msg is non-empty
                if !status_msg().is_empty() {
                    div {
                        style: "background: #101828; color: #F9FAFB; padding: 10px 12px; border-radius: 8px; font-size: 12px; margin-bottom: 12px; display: flex; justify-content: space-between; align-items: center; gap: 8px;",
                        span { "{status_msg}" }
                        button {
                            style: "background: transparent; border: none; color: #98A2B3; font-size: 14px; padding: 0 4px;",
                            onclick: move |_| status_msg.set(String::new()),
                            "×"
                        }
                    }
                }

                // Quiet permission banner only when incomplete
                if !current_health.all_granted() && active_tab() != ActiveTab::Health {
                    div {
                        style: "background: #FFFAEB; border: 1px solid #FEDF89; padding: 11px 13px; border-radius: 10px; margin-bottom: 14px; display: flex; justify-content: space-between; align-items: center; gap: 10px;",
                        div {
                            style: "font-size: 12px; color: #93370D; line-height: 1.4;",
                            "Faltam permissões do sistema ({granted_count}/6) para utilizar o Nível 3."
                        }
                        button {
                            class: "btn-secondary",
                            style: "border-color: #F79009; color: #93370D; white-space: nowrap;",
                            onclick: move |_| {
                                health_state.set(check_native_permissions());
                                active_tab.set(ActiveTab::Health);
                            },
                            "Configurar"
                        }
                    }
                }

                // TAB 1: HOJE (TIMELINE DIÁRIA)
                if active_tab() == ActiveTab::Timeline {
                    div {
                        // Calendar Card (Week Strip + toggleable Month Grid)
                        div {
                            class: "cal-card",
                            div {
                                class: "cal-top-row",
                                button {
                                    class: "cal-title-btn",
                                    onclick: move |_| show_month_picker.set(!show_month_picker()),
                                    span { if show_month_picker() { "{month_title}" } else { "{week_month_title}" } }
                                    span { style: "font-size: 10px; color: #667085;", if show_month_picker() { "▴" } else { "▾" } }
                                }
                                div {
                                    class: "cal-nav-group",
                                    if selected_day_offset() != 0 {
                                        button {
                                            class: "cal-btn-today",
                                            onclick: move |_| {
                                                selected_day_offset.set(0);
                                                refresh_timeline(0, active_tag_filter.read().clone());
                                            },
                                            "Hoje"
                                        }
                                    }
                                    button {
                                        class: "cal-nav-arrow",
                                        title: "Anterior",
                                        onclick: move |_| {
                                            let next_offset = if show_month_picker() {
                                                prev_month_offset(now_epoch, selected_day_offset())
                                            } else {
                                                selected_day_offset() - 7
                                            };
                                            selected_day_offset.set(next_offset);
                                            refresh_timeline(next_offset, active_tag_filter.read().clone());
                                        },
                                        "◀"
                                    }
                                    button {
                                        class: "cal-nav-arrow",
                                        title: "Seguinte",
                                        onclick: move |_| {
                                            let next_offset = if show_month_picker() {
                                                next_month_offset(now_epoch, selected_day_offset())
                                            } else {
                                                selected_day_offset() + 7
                                            };
                                            selected_day_offset.set(next_offset);
                                            refresh_timeline(next_offset, active_tag_filter.read().clone());
                                        },
                                        "▶"
                                    }
                                }
                            }

                            if !show_month_picker() {
                                div {
                                    class: "week-strip-grid",
                                    for day in week_days {
                                        {
                                            let day_offset = day.offset;
                                            let mut classes = String::from("cal-day-cell");
                                            if day.is_selected {
                                                classes.push_str(" is-selected");
                                            }
                                            if day.is_today {
                                                classes.push_str(" is-today");
                                            }
                                            rsx! {
                                                button {
                                                    key: "{day.offset}",
                                                    class: "{classes}",
                                                    onclick: move |_| {
                                                        selected_day_offset.set(day_offset);
                                                        refresh_timeline(day_offset, active_tag_filter.read().clone());
                                                    },
                                                    span { class: "cal-day-weekday", "{day.weekday_short}" }
                                                    span { class: "cal-day-num", "{day.day_num}" }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                div {
                                    class: "cal-month-table",
                                    for lbl in ["S", "T", "Q", "Q", "S", "S", "D"] {
                                        span { class: "cal-weekday-header", "{lbl}" }
                                    }
                                    for (idx, maybe_day) in month_cells.into_iter().enumerate() {
                                        if let Some(day) = maybe_day {
                                            {
                                                let day_offset = day.offset;
                                                let mut classes = String::from("cal-month-cell");
                                                if day.is_selected {
                                                    classes.push_str(" is-selected");
                                                }
                                                if day.is_today {
                                                    classes.push_str(" is-today");
                                                }
                                                rsx! {
                                                    button {
                                                        key: "{idx}",
                                                        class: "{classes}",
                                                        onclick: move |_| {
                                                            selected_day_offset.set(day_offset);
                                                            refresh_timeline(day_offset, active_tag_filter.read().clone());
                                                        },
                                                        "{day.day_num}"
                                                    }
                                                }
                                            }
                                        } else {
                                            span { key: "{idx}", class: "cal-month-cell is-empty" }
                                        }
                                    }
                                }
                            }
                        }

                        // Quick Note Input
                        div {
                            class: "surface-pad",
                            style: "padding: 12px;",
                            div {
                                style: "display: flex; gap: 8px;",
                                input {
                                    class: "field-input",
                                    style: "flex: 1;",
                                    r#type: "text",
                                    placeholder: "Adicionar nota rápida com #tags...",
                                    value: "{note_draft}",
                                    oninput: move |evt| note_draft.set(evt.value())
                                }
                                button {
                                    class: "btn-primary",
                                    style: "white-space: nowrap;",
                                    onclick: save_quick_note,
                                    "Guardar"
                                }
                            }
                        }

                        // Active #tag filter
                        if let Some(tag) = active_tag_filter() {
                            div {
                                style: "display: flex; justify-content: space-between; align-items: center; background: #F2F4F7; border: 1px solid #D0D5DD; padding: 8px 12px; border-radius: 8px; margin-bottom: 12px;",
                                span {
                                    style: "font-size: 12px; color: #344054;",
                                    "Filtrado por "
                                    strong { "#{tag}" }
                                }
                                button {
                                    class: "btn-secondary",
                                    style: "padding: 3px 8px; font-size: 11px;",
                                    onclick: move |_| {
                                        active_tag_filter.set(None);
                                        refresh_timeline(selected_day_offset(), None);
                                    },
                                    "Limpar"
                                }
                            }
                        }

                        // Scheduled Habits
                        div {
                            class: "section-head",
                            h2 { class: "section-title", "Agenda do dia" }
                            span { class: "section-meta", "{snap.scheduled_alarms.len()}" }
                        }
                        div {
                            class: "surface-group",
                            if snap.scheduled_alarms.is_empty() {
                                div {
                                    style: "padding: 20px; text-align: center; color: #667085; font-size: 13px;",
                                    "Nenhum hábito agendado para esta data."
                                }
                            }
                            for item in snap.scheduled_alarms.iter() {
                                {
                                    let id_for_test = item.id.clone();
                                    let level_label = match item.level {
                                        AlarmLevel::Passive => "Nível 1",
                                        AlarmLevel::Persistent => "Nível 2",
                                        AlarmLevel::ProofOfAction => "Nível 3",
                                    };
                                    rsx! {
                                        div {
                                            key: "sched-{item.id}",
                                            class: "list-row",
                                            div {
                                                style: "display: flex; align-items: center; gap: 14px; min-width: 0;",
                                                div {
                                                    class: "time-display",
                                                    "{item.hour:02}:{item.minute:02}"
                                                }
                                                div {
                                                    style: "min-width: 0;",
                                                    div {
                                                        style: "font-size: 15px; font-weight: 500; color: #101828; white-space: nowrap; overflow: hidden; text-overflow: ellipsis;",
                                                        "{item.title}"
                                                    }
                                                    div {
                                                        style: "font-size: 12px; color: #667085; margin-top: 2px;",
                                                        "{level_label} · Janela {item.grace_window_minutes} min"
                                                        if item.level == AlarmLevel::ProofOfAction {
                                                            " · Objeto: {item.target_label}"
                                                        }
                                                    }
                                                }
                                            }
                                            button {
                                                class: "btn-secondary",
                                                style: "flex-shrink: 0;",
                                                onclick: move |_| {
                                                    let test_trigger = current_epoch_ms() + 10_000;
                                                    match schedule_native_alarm(&id_for_test, test_trigger) {
                                                        Ok(()) => status_msg.set(format!("Alarme agendado para daqui a 10s.")),
                                                        Err(err) => status_msg.set(format!("Falha JNI: {err}")),
                                                    }
                                                },
                                                "Testar +10s"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Recorded Outcomes
                        div {
                            class: "section-head",
                            h2 { class: "section-title", "Execuções registadas" }
                            span { class: "section-meta", "{snap.outcomes.len()}" }
                        }
                        div {
                            class: "surface-group",
                            if snap.outcomes.is_empty() {
                                div {
                                    style: "padding: 20px; text-align: center; color: #667085; font-size: 13px;",
                                    "Sem execuções registadas nesta data."
                                }
                            }
                            for out in snap.outcomes.iter() {
                                {
                                    let time_str = format_local_time_hms(out.recorded_at_epoch_ms);
                                    let (badge_bg, badge_fg, label_txt) = match out.outcome.as_str() {
                                        "CUMPRIDO" => ("#ECFDF3", "#027A48", "Cumprido"),
                                        "FALHOU" => ("#FEF3F2", "#B42318", "Falhou"),
                                        "ADIADO" => ("#FFFAEB", "#B54708", "Adiado"),
                                        _ => ("#F2F4F7", "#344054", out.outcome.as_str()),
                                    };
                                    rsx! {
                                        div {
                                            key: "tl-out-{out.id}",
                                            class: "list-row",
                                            style: "flex-direction: column; align-items: stretch; gap: 4px;",
                                            div {
                                                style: "display: flex; justify-content: space-between; align-items: center;",
                                                div {
                                                    style: "display: flex; align-items: baseline; gap: 8px;",
                                                    span {
                                                        style: "font-size: 12px; color: #667085; font-variant-numeric: tabular-nums;",
                                                        "{time_str}"
                                                    }
                                                    span {
                                                        style: "font-size: 14px; font-weight: 500; color: #101828;",
                                                        "{out.alarm_title}"
                                                    }
                                                }
                                                span {
                                                    style: "padding: 2px 8px; background: {badge_bg}; color: {badge_fg}; border-radius: 999px; font-size: 11px; font-weight: 500;",
                                                    "{label_txt}"
                                                }
                                            }
                                            if let Some(lbl) = &out.matched_label {
                                                div {
                                                    style: "font-size: 12px; color: #475467;",
                                                    if let Some(conf) = out.confidence {
                                                        "Objeto validado: {lbl} ({conf * 100.0:.0}%)"
                                                    } else {
                                                        "Objeto validado: {lbl}"
                                                    }
                                                }
                                            }
                                            if let Some(path) = &out.proof_image_path {
                                                div {
                                                    style: "font-size: 11px; color: #667085; word-break: break-all;",
                                                    "Ficheiro: {path}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Quick Notes
                        div {
                            class: "section-head",
                            h2 { class: "section-title", "Notas" }
                            span { class: "section-meta", "{snap.notes.len()}" }
                        }
                        div {
                            class: "surface-group",
                            if snap.notes.is_empty() {
                                div {
                                    style: "padding: 20px; text-align: center; color: #667085; font-size: 13px;",
                                    "Sem notas registadas nesta data."
                                }
                            }
                            for note in snap.notes.iter() {
                                {
                                    let note_id = note.id.clone();
                                    let time_str = format_local_time_hm(note.created_at_epoch_ms);
                                    rsx! {
                                        div {
                                            key: "note-{note.id}",
                                            class: "list-row",
                                            style: "flex-direction: column; align-items: stretch; gap: 6px;",
                                            div {
                                                style: "display: flex; justify-content: space-between; align-items: center;",
                                                span {
                                                    style: "font-size: 12px; color: #667085; font-variant-numeric: tabular-nums;",
                                                    "{time_str}"
                                                }
                                                button {
                                                    style: "background: transparent; border: none; color: #667085; font-size: 12px; padding: 0;",
                                                    onclick: move |_| {
                                                        let db = resolve_db_path();
                                                        let now_ms = current_epoch_ms();
                                                        if let Ok(store) = CoreStore::open(db) {
                                                            if store.delete_note(&note_id, now_ms).is_ok() {
                                                                status_msg.set(String::from("Nota removida."));
                                                                refresh_timeline(selected_day_offset(), active_tag_filter.read().clone());
                                                            }
                                                        }
                                                    },
                                                    "Remover"
                                                }
                                            }
                                            div {
                                                style: "font-size: 14px; color: #101828; line-height: 1.45;",
                                                "{note.body}"
                                            }
                                            if !note.tags.is_empty() {
                                                div {
                                                    style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                                    for t in note.tags.iter() {
                                                        {
                                                            let tag_owned = t.clone();
                                                            rsx! {
                                                                button {
                                                                    style: "padding: 2px 8px; background: #F2F4F7; color: #344054; border: 1px solid #E4E7EC; border-radius: 999px; font-size: 11px; font-weight: 500;",
                                                                    onclick: move |_| {
                                                                        active_tag_filter.set(Some(tag_owned.clone()));
                                                                        refresh_timeline(selected_day_offset(), Some(tag_owned.clone()));
                                                                    },
                                                                    "#{t}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // TAB 2: HÁBITOS & ALARMES
                if active_tab() == ActiveTab::Habits {
                    div {
                        if permission_blocked_n3() {
                            div {
                                style: "background: #FEF3F2; border: 1px solid #FECDCA; padding: 12px; border-radius: 10px; margin-bottom: 14px; display: flex; justify-content: space-between; align-items: center; gap: 10px;",
                                span {
                                    style: "font-size: 12px; color: #B42318;",
                                    "Conceda as 6 permissões antes de ativar um hábito Nível 3."
                                }
                                button {
                                    class: "btn-secondary",
                                    onclick: move |_| {
                                        health_state.set(check_native_permissions());
                                        active_tab.set(ActiveTab::Health);
                                    },
                                    "Permissões"
                                }
                            }
                        }

                        if let Some(blocked_id) = cooldown_blocked_id() {
                            div {
                                style: "background: #FFFAEB; border: 1px solid #FEDF89; padding: 12px; border-radius: 10px; margin-bottom: 14px; display: flex; justify-content: space-between; align-items: center; gap: 10px;",
                                span {
                                    style: "font-size: 12px; color: #93370D;",
                                    "Hábito Nível 3 protegido por bloqueio de 24 horas."
                                }
                                button {
                                    class: "btn-secondary",
                                    onclick: move |_| {
                                        let db = resolve_db_path();
                                        let now_ms = current_epoch_ms();
                                        if let Ok(store) = CoreStore::open(db) {
                                            match store.request_cooldown_unlock(&blocked_id, now_ms) {
                                                Ok(unlock_at) => {
                                                    status_msg.set(format!("Desbloqueio agendado para daqui a 24h ({unlock_at})."));
                                                    cooldown_blocked_id.set(None);
                                                    reload_all(None);
                                                }
                                                Err(err) => status_msg.set(format!("Erro: {err}")),
                                            }
                                        }
                                    },
                                    "Solicitar desbloqueio (24h)"
                                }
                            }
                        }

                        // Habit Editor Card
                        div {
                            class: "surface-pad",
                            h2 {
                                style: "margin: 0 0 12px 0; font-size: 15px; font-weight: 600; color: #101828;",
                                if editing_id().is_some() { "Editar hábito" } else { "Novo hábito" }
                            }

                            div {
                                style: "margin-bottom: 12px;",
                                label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 4px;", "Nome" }
                                input {
                                    class: "field-input",
                                    r#type: "text",
                                    value: "{title}",
                                    oninput: move |evt| title.set(evt.value())
                                }
                            }

                            div {
                                style: "display: flex; gap: 10px; margin-bottom: 12px;",
                                div {
                                    style: "flex: 1;",
                                    label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 4px;", "Hora" }
                                    input {
                                        class: "field-input",
                                        r#type: "number",
                                        min: "0",
                                        max: "23",
                                        value: "{hour}",
                                        oninput: move |evt| {
                                            if let Ok(v) = evt.value().parse::<u8>() {
                                                hour.set(v.min(23));
                                            }
                                        }
                                    }
                                }
                                div {
                                    style: "flex: 1;",
                                    label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 4px;", "Minuto" }
                                    input {
                                        class: "field-input",
                                        r#type: "number",
                                        min: "0",
                                        max: "59",
                                        value: "{minute}",
                                        oninput: move |evt| {
                                            if let Ok(v) = evt.value().parse::<u8>() {
                                                minute.set(v.min(59));
                                            }
                                        }
                                    }
                                }
                                div {
                                    style: "flex: 1;",
                                    label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 4px;", "Janela (min)" }
                                    input {
                                        class: "field-input",
                                        r#type: "number",
                                        min: "1",
                                        max: "240",
                                        value: "{grace_window_minutes}",
                                        oninput: move |evt| {
                                            if let Ok(v) = evt.value().parse::<u16>() {
                                                grace_window_minutes.set(v.clamp(1, 240));
                                            }
                                        }
                                    }
                                }
                            }

                            div {
                                style: "margin-bottom: 12px;",
                                label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 6px;", "Dias da semana" }
                                div {
                                    style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                    for (name, bit) in WEEKDAY_NAMES.iter().copied() {
                                        button {
                                            style: if (weekdays_mask() & bit) != 0 {
                                                "padding: 6px 10px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 6px; font-size: 12px; font-weight: 500;"
                                            } else {
                                                "padding: 6px 10px; background: #FFFFFF; color: #475467; border: 1px solid #D0D5DD; border-radius: 6px; font-size: 12px; font-weight: 500;"
                                            },
                                            onclick: move |_| weekdays_mask.set(weekdays_mask() ^ bit),
                                            "{name}"
                                        }
                                    }
                                }
                            }

                            div {
                                style: "margin-bottom: 12px;",
                                label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 6px;", "Nível de exigência" }
                                div {
                                    style: "display: flex; gap: 6px;",
                                    for (lvl, label_txt) in [
                                        (AlarmLevel::Passive, "Nível 1 · Leve"),
                                        (AlarmLevel::Persistent, "Nível 2 · Médio"),
                                        (AlarmLevel::ProofOfAction, "Nível 3 · Câmara"),
                                    ] {
                                        button {
                                            style: if level() == lvl {
                                                "flex: 1; padding: 8px 6px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 7px; font-weight: 500; font-size: 12px;"
                                            } else {
                                                "flex: 1; padding: 8px 6px; background: #FFFFFF; color: #475467; border: 1px solid #D0D5DD; border-radius: 7px; font-weight: 500; font-size: 12px;"
                                            },
                                            onclick: move |_| level.set(lvl),
                                            "{label_txt}"
                                        }
                                    }
                                }
                            }

                            if level() == AlarmLevel::ProofOfAction {
                                div {
                                    style: "margin-bottom: 14px; background: #F9FAFB; border: 1px solid #EAECF0; padding: 10px; border-radius: 8px;",
                                    label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 6px;", "Objeto para validação visual" }
                                    div {
                                        style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                        for (code, friendly) in CURATED_OBJECT_LABELS.iter().copied() {
                                            button {
                                                style: if target_label() == code {
                                                    "padding: 5px 10px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 6px; font-size: 12px; font-weight: 500;"
                                                } else {
                                                    "padding: 5px 10px; background: #FFFFFF; color: #344054; border: 1px solid #D0D5DD; border-radius: 6px; font-size: 12px; font-weight: 500;"
                                                },
                                                onclick: move |_| target_label.set(code.to_string()),
                                                "{friendly}"
                                            }
                                        }
                                    }
                                }
                            }

                            div {
                                style: "display: flex; gap: 8px;",
                                button {
                                    class: "btn-primary",
                                    style: "flex: 1;",
                                    onclick: save_alarm,
                                    if editing_id().is_some() { "Guardar alterações" } else { "Adicionar hábito" }
                                }
                                if editing_id().is_some() {
                                    button {
                                        class: "btn-secondary",
                                        onclick: move |_| reset_form(),
                                        "Cancelar"
                                    }
                                }
                            }
                        }

                        // Habits List
                        div {
                            class: "section-head",
                            h2 { class: "section-title", "Todos os hábitos" }
                            span { class: "section-meta", "{alarms().len()}" }
                        }
                        div {
                            class: "surface-group",
                            if alarms().is_empty() {
                                div {
                                    style: "padding: 20px; text-align: center; color: #667085; font-size: 13px;",
                                    "Nenhum hábito configurado."
                                }
                            }
                            for item in alarms() {
                                {
                                    let id_for_toggle = item.id.clone();
                                    let item_level = item.level;
                                    let next_enabled = !item.enabled;
                                    let item_for_edit = item.clone();
                                    let id_for_delete = item.id.clone();
                                    let id_for_test = item.id.clone();
                                    let id_for_outcomes = item.id.clone();
                                    let days_str = format_weekdays(item.weekdays_mask);

                                    rsx! {
                                        div {
                                            key: "{item.id}",
                                            class: "list-row",
                                            style: "flex-direction: column; align-items: stretch; gap: 10px;",
                                            div {
                                                style: "display: flex; justify-content: space-between; align-items: flex-start; gap: 12px;",
                                                div {
                                                    style: "display: flex; align-items: center; gap: 12px;",
                                                    div {
                                                        class: "time-display",
                                                        style: if item.enabled { "" } else { "color: #98A2B3;" },
                                                        "{item.hour:02}:{item.minute:02}"
                                                    }
                                                    div {
                                                        div {
                                                            style: "font-size: 15px; font-weight: 500; color: #101828;",
                                                            "{item.title}"
                                                        }
                                                        div {
                                                            style: "font-size: 12px; color: #667085; margin-top: 2px;",
                                                            "Nível {item.level.as_i32()} · {days_str} · {item.grace_window_minutes} min"
                                                            if item.level == AlarmLevel::ProofOfAction {
                                                                " · {item.target_label}"
                                                            }
                                                        }
                                                        if let Some(unlock_ts) = item.cooldown_unlock_at_epoch_ms {
                                                            div {
                                                                style: "font-size: 11px; color: #B54708; margin-top: 2px;",
                                                                "Desbloqueio agendado: {unlock_ts}"
                                                            }
                                                        }
                                                    }
                                                }
                                                span {
                                                    style: if item.enabled {
                                                        "font-size: 11px; padding: 2px 8px; background: #ECFDF3; color: #027A48; border-radius: 999px; font-weight: 500;"
                                                    } else {
                                                        "font-size: 11px; padding: 2px 8px; background: #F2F4F7; color: #667085; border-radius: 999px; font-weight: 500;"
                                                    },
                                                    if item.enabled { "Ativo" } else { "Pausado" }
                                                }
                                            }

                                            div {
                                                style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                                button {
                                                    class: "btn-secondary",
                                                    onclick: move |_| {
                                                        if next_enabled && item_level == AlarmLevel::ProofOfAction {
                                                            let h = check_native_permissions();
                                                            health_state.set(h.clone());
                                                            if !h.all_granted() {
                                                                permission_blocked_n3.set(true);
                                                                status_msg.set(format!(
                                                                    "Permissões pendentes para Nível 3: {}.",
                                                                    h.missing_labels().join(", ")
                                                                ));
                                                                return;
                                                            }
                                                        }
                                                        let db = resolve_db_path();
                                                        let now_ms = current_epoch_ms();
                                                        if let Ok(store) = CoreStore::open(db) {
                                                            match store.set_alarm_enabled(&id_for_toggle, next_enabled, now_ms) {
                                                                Ok(updated) => {
                                                                    let _ = if updated.enabled {
                                                                        schedule_native_alarm(&updated.id, updated.trigger_at_epoch_ms)
                                                                    } else {
                                                                        cancel_native_alarm(&updated.id)
                                                                    };
                                                                    status_msg.set(format!("Estado de '{}' atualizado.", updated.title));
                                                                    reload_all(Some(updated.id));
                                                                }
                                                                Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                                                                    cooldown_blocked_id.set(Some(id_for_toggle.clone()));
                                                                    status_msg.set(format!("Bloqueio 24h ativo até {unlock_at_epoch_ms}"));
                                                                }
                                                                Err(err) => status_msg.set(format!("Erro: {err}")),
                                                            }
                                                        }
                                                    },
                                                    if item.enabled { "Pausar" } else { "Ativar" }
                                                }

                                                button {
                                                    class: "btn-secondary",
                                                    onclick: move |_| {
                                                        editing_id.set(Some(item_for_edit.id.clone()));
                                                        title.set(item_for_edit.title.clone());
                                                        hour.set(item_for_edit.hour);
                                                        minute.set(item_for_edit.minute);
                                                        weekdays_mask.set(item_for_edit.weekdays_mask);
                                                        level.set(item_for_edit.level);
                                                        if !item_for_edit.target_label.is_empty() {
                                                            target_label.set(item_for_edit.target_label.clone());
                                                        }
                                                        grace_window_minutes.set(item_for_edit.grace_window_minutes);
                                                        enabled.set(item_for_edit.enabled);
                                                    },
                                                    "Editar"
                                                }

                                                button {
                                                    class: "btn-secondary",
                                                    onclick: move |_| {
                                                        let test_trigger = current_epoch_ms() + 10_000;
                                                        match schedule_native_alarm(&id_for_test, test_trigger) {
                                                            Ok(()) => status_msg.set(format!("Teste agendado para +10s.")),
                                                            Err(err) => status_msg.set(format!("Erro JNI: {err}")),
                                                        }
                                                    },
                                                    "Testar +10s"
                                                }

                                                button {
                                                    class: "btn-secondary",
                                                    onclick: move |_| reload_all(Some(id_for_outcomes.clone())),
                                                    "Histórico"
                                                }

                                                button {
                                                    class: "btn-danger-ghost",
                                                    onclick: move |_| {
                                                        let db = resolve_db_path();
                                                        let now_ms = current_epoch_ms();
                                                        if let Ok(store) = CoreStore::open(db) {
                                                            match store.delete_alarm(&id_for_delete, now_ms) {
                                                                Ok(()) => {
                                                                    let _ = cancel_native_alarm(&id_for_delete);
                                                                    status_msg.set(String::from("Hábito removido."));
                                                                    reload_all(None);
                                                                }
                                                                Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                                                                    cooldown_blocked_id.set(Some(id_for_delete.clone()));
                                                                    status_msg.set(format!("Bloqueio 24h ativo até {unlock_at_epoch_ms}"));
                                                                }
                                                                Err(err) => status_msg.set(format!("Erro: {err}")),
                                                            }
                                                        }
                                                    },
                                                    "Remover"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        if let Some(sel_id) = selected_alarm_id() {
                            div {
                                class: "surface-pad",
                                h3 { style: "margin: 0 0 8px 0; font-size: 14px; font-weight: 600;", "Histórico ({sel_id})" }
                                if outcomes().is_empty() {
                                    p { style: "font-size: 12px; color: #667085; margin: 0;", "Sem registos para este hábito." }
                                }
                                for out in outcomes() {
                                    div {
                                        key: "{out.id}",
                                        style: "font-size: 12px; padding: 6px 0; border-bottom: 1px solid #F2F4F7; color: #344054;",
                                        div {
                                            "{format_local_time_hms(out.recorded_at_epoch_ms)} · {out.outcome}"
                                            if let Some(lbl) = &out.matched_label {
                                                if let Some(conf) = out.confidence {
                                                    " · {lbl} ({conf * 100.0:.0}%)"
                                                } else {
                                                    " · {lbl}"
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // TAB 3: PERMISSÕES DO SISTEMA
                if active_tab() == ActiveTab::Health {
                    div {
                        div {
                            class: "surface-pad",
                            style: "display: flex; justify-content: space-between; align-items: center; gap: 12px;",
                            div {
                                div {
                                    style: "font-size: 15px; font-weight: 600; color: #101828;",
                                    "Permissões do Android ({granted_count}/6)"
                                }
                                p {
                                    style: "margin: 3px 0 0 0; font-size: 12px; color: #667085; line-height: 1.4;",
                                    "Necessárias para disparo exato, sobreposição de ecrã e validação por câmara."
                                }
                            }
                            button {
                                class: "btn-secondary",
                                onclick: move |_| {
                                    let refreshed = check_native_permissions();
                                    if refreshed.all_granted() {
                                        permission_blocked_n3.set(false);
                                    }
                                    health_state.set(refreshed);
                                    status_msg.set(String::from("Permissões atualizadas."));
                                },
                                "Atualizar"
                            }
                        }

                        div {
                            class: "surface-group",
                            for (key, name, desc, is_ok) in [
                                (
                                    "exact_alarm",
                                    "Alarme exato",
                                    "Dispara no minuto agendado mesmo com o ecrã em repouso.",
                                    current_health.exact_alarm,
                                ),
                                (
                                    "overlay",
                                    "Sobrepor a outras aplicações",
                                    "Permite abrir o ecrã de alarme sobre qualquer aplicação.",
                                    current_health.overlay,
                                ),
                                (
                                    "accessibility",
                                    "Serviço de acessibilidade",
                                    "Mantém o ecrã de validação focado durante alarmes Nível 3.",
                                    current_health.accessibility,
                                ),
                                (
                                    "camera",
                                    "Câmara",
                                    "Utilizada localmente para reconhecer o objeto do hábito.",
                                    current_health.camera,
                                ),
                                (
                                    "notifications",
                                    "Notificações de ecrã inteiro",
                                    "Mantém o serviço de som ativo e acorda o dispositivo.",
                                    current_health.notifications,
                                ),
                                (
                                    "battery",
                                    "Sem restrição de bateria",
                                    "Evita que o sistema interrompa o agendamento em segundo plano.",
                                    current_health.battery,
                                ),
                            ] {
                                div {
                                    key: "perm-{key}",
                                    class: "list-row",
                                    div {
                                        div {
                                            style: "display: flex; align-items: center; gap: 8px;",
                                            span {
                                                class: if is_ok { "dot-ok" } else { "dot-warn" }
                                            }
                                            span {
                                                style: "font-size: 14px; font-weight: 500; color: #101828;",
                                                "{name}"
                                            }
                                        }
                                        div {
                                            style: "font-size: 12px; color: #667085; margin-top: 2px; padding-left: 14px;",
                                            "{desc}"
                                        }
                                    }
                                    button {
                                        class: if is_ok { "btn-secondary" } else { "btn-primary" },
                                        style: "padding: 6px 11px; font-size: 12px; flex-shrink: 0;",
                                        onclick: move |_| {
                                            match open_native_permission_setting(key) {
                                                Ok(()) => status_msg.set(format!("A abrir '{name}'...")),
                                                Err(err) => status_msg.set(format!("Erro JNI: {err}")),
                                            }
                                        },
                                        if is_ok { "Rever" } else { "Ativar" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Fixed Bottom Navigation Bar
            nav {
                class: "bottom-nav",
                button {
                    class: if active_tab() == ActiveTab::Timeline { "nav-item nav-item-active" } else { "nav-item" },
                    onclick: move |_| {
                        active_tab.set(ActiveTab::Timeline);
                        reload_all(None);
                    },
                    svg {
                        width: "18",
                        height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        rect { x: "3", y: "4", width: "18", height: "18", rx: "2", ry: "2" }
                        line { x1: "16", y1: "2", x2: "16", y2: "6" }
                        line { x1: "8", y1: "2", x2: "8", y2: "6" }
                        line { x1: "3", y1: "10", x2: "21", y2: "10" }
                    }
                    span { "Hoje" }
                }
                button {
                    class: if active_tab() == ActiveTab::Habits { "nav-item nav-item-active" } else { "nav-item" },
                    onclick: move |_| {
                        active_tab.set(ActiveTab::Habits);
                        reload_all(None);
                    },
                    svg {
                        width: "18",
                        height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        circle { cx: "12", cy: "13", r: "8" }
                        path { d: "M12 9v4l2 2" }
                        path { d: "M5 3L2 6" }
                        path { d: "M22 6l-3-3" }
                    }
                    span { "Hábitos" }
                }
                button {
                    class: if active_tab() == ActiveTab::Health { "nav-item nav-item-active" } else { "nav-item" },
                    onclick: move |_| {
                        health_state.set(check_native_permissions());
                        active_tab.set(ActiveTab::Health);
                    },
                    svg {
                        width: "18",
                        height: "18",
                        view_box: "0 0 24 24",
                        fill: "none",
                        stroke: "currentColor",
                        stroke_width: "1.8",
                        stroke_linecap: "round",
                        stroke_linejoin: "round",
                        path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
                    }
                    span { "Sistema" }
                }
            }
        }
    }
}

fn format_weekdays(mask: u8) -> String {
    if mask == 0 {
        return String::from("Evento Único");
    }
    if mask == 0b0111_1111 {
        return String::from("Todos os dias");
    }
    let mut parts = Vec::new();
    for (name, bit) in WEEKDAY_NAMES.iter().copied() {
        if (mask & bit) != 0 {
            parts.push(name);
        }
    }
    parts.join(", ")
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

#[cfg(target_os = "android")]
fn resolve_db_path() -> &'static str {
    DB_PATH_CACHE.get_or_init(|| {
        android_files_dir()
            .map(|dir| format!("{dir}/crabcheck.db"))
            .unwrap_or_else(|_| String::from("/data/local/tmp/crabcheck.db"))
    })
}

#[cfg(not(target_os = "android"))]
fn resolve_db_path() -> &'static str {
    DB_PATH_CACHE.get_or_init(|| {
        std::env::temp_dir()
            .join("crabcheck_spike.db")
            .to_string_lossy()
            .into_owned()
    })
}

#[cfg(target_os = "android")]
fn clear_jni_exception(env: &mut jni::JNIEnv<'_>, err: jni::errors::Error) -> String {
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    err.to_string()
}

#[cfg(target_os = "android")]
fn android_files_dir() -> Result<String, String> {
    use jni::objects::{JObject, JString};
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let context_obj = unsafe { JObject::from_raw(ctx.context().cast()) };

    let file_val = env
        .call_method(&context_obj, "getFilesDir", "()Ljava/io/File;", &[])
        .map_err(|e| clear_jni_exception(&mut env, e))?;
    let file_obj = file_val.l().map_err(|e| clear_jni_exception(&mut env, e))?;

    let path_val = env
        .call_method(file_obj, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .map_err(|e| clear_jni_exception(&mut env, e))?;
    let path_jstr: JString = path_val
        .l()
        .map_err(|e| clear_jni_exception(&mut env, e))?
        .into();

    let path_rust: String = env
        .get_string(&path_jstr)
        .map_err(|e| clear_jni_exception(&mut env, e))?
        .into();
    Ok(path_rust)
}

#[cfg(target_os = "android")]
fn with_alarm_scheduler_class<F, R>(f: F) -> Result<R, String>
where
    F: FnOnce(
        &mut jni::JNIEnv<'_>,
        &jni::objects::JObject<'_>,
        jni::objects::JClass<'_>,
    ) -> Result<R, String>,
{
    use jni::objects::{JObject, JValue};
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let context_obj = unsafe { JObject::from_raw(ctx.context().cast()) };

    let loader_val = env
        .call_method(
            &context_obj,
            "getClassLoader",
            "()Ljava/lang/ClassLoader;",
            &[],
        )
        .map_err(|e| clear_jni_exception(&mut env, e))?;
    let class_loader = loader_val
        .l()
        .map_err(|e| clear_jni_exception(&mut env, e))?;

    let class_name = env
        .new_string("com.crabcheck.alarmlib.AlarmScheduler")
        .map_err(|e| clear_jni_exception(&mut env, e))?;

    let scheduler_val = env
        .call_method(
            class_loader,
            "loadClass",
            "(Ljava/lang/String;)Ljava/lang/Class;",
            &[JValue::Object(&class_name)],
        )
        .map_err(|e| clear_jni_exception(&mut env, e))?;
    let scheduler_class_obj = scheduler_val
        .l()
        .map_err(|e| clear_jni_exception(&mut env, e))?;

    let scheduler_class: jni::objects::JClass = scheduler_class_obj.into();
    f(&mut env, &context_obj, scheduler_class)
}

#[cfg(target_os = "android")]
fn schedule_native_alarm(alarm_id: &str, trigger_at_epoch_ms: i64) -> Result<(), String> {
    use jni::objects::JValue;
    with_alarm_scheduler_class(|env, context_obj, scheduler_class| {
        let alarm_id_jstr = env
            .new_string(alarm_id)
            .map_err(|e| clear_jni_exception(env, e))?;

        env.call_static_method(
            scheduler_class,
            "scheduleExactAlarm",
            "(Landroid/content/Context;Ljava/lang/String;J)V",
            &[
                JValue::Object(context_obj),
                JValue::Object(&alarm_id_jstr),
                JValue::Long(trigger_at_epoch_ms),
            ],
        )
        .map_err(|e| clear_jni_exception(env, e))?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
fn cancel_native_alarm(alarm_id: &str) -> Result<(), String> {
    use jni::objects::JValue;
    with_alarm_scheduler_class(|env, context_obj, scheduler_class| {
        let alarm_id_jstr = env
            .new_string(alarm_id)
            .map_err(|e| clear_jni_exception(env, e))?;

        env.call_static_method(
            scheduler_class,
            "cancelExactAlarm",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(context_obj), JValue::Object(&alarm_id_jstr)],
        )
        .map_err(|e| clear_jni_exception(env, e))?;
        Ok(())
    })
}

#[cfg(target_os = "android")]
fn check_native_permissions() -> PermissionHealthState {
    use jni::objects::{JString, JValue};
    let res = with_alarm_scheduler_class(|env, context_obj, scheduler_class| {
        let val = env
            .call_static_method(
                scheduler_class,
                "checkPermissionsCsv",
                "(Landroid/content/Context;)Ljava/lang/String;",
                &[JValue::Object(context_obj)],
            )
            .map_err(|e| clear_jni_exception(env, e))?;
        let jstr: JString = val.l().map_err(|e| clear_jni_exception(env, e))?.into();
        let rust_str: String = env
            .get_string(&jstr)
            .map_err(|e| clear_jni_exception(env, e))?
            .into();
        Ok(PermissionHealthState::from_csv(&rust_str))
    });
    res.unwrap_or_else(|_| PermissionHealthState::all_granted_default())
}

#[cfg(target_os = "android")]
fn open_native_permission_setting(key: &str) -> Result<(), String> {
    use jni::objects::JValue;
    with_alarm_scheduler_class(|env, context_obj, scheduler_class| {
        let key_jstr = env
            .new_string(key)
            .map_err(|e| clear_jni_exception(env, e))?;

        env.call_static_method(
            scheduler_class,
            "openPermissionSetting",
            "(Landroid/content/Context;Ljava/lang/String;)V",
            &[JValue::Object(context_obj), JValue::Object(&key_jstr)],
        )
        .map_err(|e| clear_jni_exception(env, e))?;
        Ok(())
    })
}

#[cfg(not(target_os = "android"))]
fn schedule_native_alarm(_alarm_id: &str, _trigger_at_epoch_ms: i64) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn cancel_native_alarm(_alarm_id: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn check_native_permissions() -> PermissionHealthState {
    PermissionHealthState::all_granted_default()
}

#[cfg(not(target_os = "android"))]
fn open_native_permission_setting(_key: &str) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calendar_week_strip() {
        // Fix a known epoch: 2026-10-23T12:00:00Z is a Friday
        let fixed_epoch_ms = 1792756800000;
        let (header, days) = calculate_week_strip(fixed_epoch_ms, 0);
        assert!(!header.is_empty());
        assert_eq!(days.len(), 7);
        // At offset 0, exactly one day is today and selected
        let today_count = days.iter().filter(|d| d.is_today).count();
        let selected_count = days.iter().filter(|d| d.is_selected).count();
        assert_eq!(today_count, 1);
        assert_eq!(selected_count, 1);
    }

    #[test]
    fn test_calendar_month_grid_and_offsets() {
        let fixed_epoch_ms = 1792756800000;
        let (header, cells) = calculate_month_grid(fixed_epoch_ms, 0);
        assert!(header.contains("2026"));
        assert!(!cells.is_empty());

        let prev = prev_month_offset(fixed_epoch_ms, 0);
        let next = next_month_offset(fixed_epoch_ms, 0);
        assert!(prev < 0);
        assert!(next > 0);
    }
}
