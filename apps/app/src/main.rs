#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::OnceLock;

use crabcheck_core::{
    day_reference_epoch_ms, AlarmDraft, AlarmLevel, AlarmSpec, CoreError, CoreStore,
    DailyTimelineSnapshot, StreakSummary,
};
use dioxus::prelude::*;

mod components;
mod styles;

use components::*;
use styles::GLOBAL_CSS;

// Ensure crabcheck-core-ffi symbols remain linked into the main binary on Android
#[used]
static KEEP_FFI: fn(
    String,
    String,
) -> Result<crabcheck_core_ffi::FfiAlarmDto, crabcheck_core_ffi::FfiError> =
    crabcheck_core_ffi::ffi_get_alarm;

static DB_PATH_CACHE: OnceLock<String> = OnceLock::new();

const CRABCHECK_LOGO_DATA_URI: &str = include_str!("../assets/crab_b64.txt");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActiveTab {
    Timeline,
    Habits,
    Health,
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

fn default_streak_summary() -> StreakSummary {
    StreakSummary {
        current_streak_days: 0,
        best_streak_days: 0,
        total_completed: 0,
        total_failed: 0,
        adherence_rate: 100,
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

    // Timeline & Calendar signals
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

    let mut streak_summary = use_signal(|| {
        let db = resolve_db_path();
        let now = current_epoch_ms();
        CoreStore::open(db)
            .and_then(|s| s.calculate_streak_summary(now))
            .unwrap_or_else(|_| default_streak_summary())
    });

    let mut day_outcomes_map = use_signal(|| {
        let db = resolve_db_path();
        let now = current_epoch_ms();
        // 60 days before and 60 days after
        let range = 60 * 86_400_000i64;
        CoreStore::open(db)
            .and_then(|s| s.get_day_outcomes_map(now - range, now + range))
            .unwrap_or_default()
    });

    // Habits signals
    let mut alarms = use_signal(|| {
        let db = resolve_db_path();
        CoreStore::open(db)
            .and_then(|s| s.list_alarms())
            .unwrap_or_default()
    });
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
        let now = current_epoch_ms();
        let ref_ms = day_reference_epoch_ms(now, day_offset);
        if let Ok(store) = CoreStore::open(db) {
            if let Ok(snap) = store.daily_timeline(ref_ms, tag_filter.as_deref()) {
                timeline_snapshot.set(snap);
            }
            if let Ok(summary) = store.calculate_streak_summary(now) {
                streak_summary.set(summary);
            }
            let range = 60 * 86_400_000i64;
            if let Ok(map) = store.get_day_outcomes_map(now - range, now + range) {
                day_outcomes_map.set(map);
            }
        }
    };

    let mut reload_all = move |_maybe_select_id: Option<String>| {
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
                let now = current_epoch_ms();
                let ref_ms = day_reference_epoch_ms(now, selected_day_offset());
                let filter = active_tag_filter.read().clone();
                if let Ok(snap) = store.daily_timeline(ref_ms, filter.as_deref()) {
                    timeline_snapshot.set(snap);
                }
                if let Ok(summary) = store.calculate_streak_summary(now) {
                    streak_summary.set(summary);
                }
                let range = 60 * 86_400_000i64;
                if let Ok(map) = store.get_day_outcomes_map(now - range, now + range) {
                    day_outcomes_map.set(map);
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

    let mut save_alarm = move |_| {
        let current_health = check_native_permissions();
        health_state.set(current_health.clone());

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

    let mut save_quick_note = move |_| {
        let text = note_draft();
        if text.trim().is_empty() {
            return;
        }
        let db = resolve_db_path();
        let now_ms = current_epoch_ms();
        match CoreStore::open(db).and_then(|s| s.create_note(&text, now_ms)) {
            Ok(_) => {
                note_draft.set(String::new());
                status_msg.set(String::from("Nota guardada na linha do tempo."));
                refresh_timeline(selected_day_offset(), active_tag_filter.read().clone());
            }
            Err(err) => status_msg.set(format!("Erro ao guardar nota: {err}")),
        }
    };

    let snap = timeline_snapshot();
    let current_health = health_state();
    let granted_count = current_health.granted_count();
    let now_epoch = current_epoch_ms();
    let outcomes_map_data = day_outcomes_map();

    rsx! {
        div {
            class: "app-shell",
            style { "{GLOBAL_CSS}" }

            // Sticky Top Bar
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

                // Subtle dismissible toast
                if !status_msg().is_empty() {
                    div {
                        style: "background: #101828; color: #F9FAFB; padding: 10px 12px; border-radius: 8px; font-size: 12px; margin-bottom: 12px; display: flex; justify-content: space-between; align-items: center; gap: 8px;",
                        span { "{status_msg}" }
                        button {
                            style: "background: transparent; border: none; color: #98A2B3; font-size: 14px; padding: 0 4px; cursor: pointer;",
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
                            style: "border-color: #F79009; color: #93370D; white-space: nowrap; cursor: pointer;",
                            onclick: move |_| {
                                health_state.set(check_native_permissions());
                                active_tab.set(ActiveTab::Health);
                            },
                            "Configurar"
                        }
                    }
                }

                // TAB 1: HOJE (TIMELINE + CALENDÁRIO)
                if active_tab() == ActiveTab::Timeline {
                    div {
                        CalendarCard {
                            now_epoch_ms: now_epoch,
                            selected_offset: selected_day_offset(),
                            show_month: show_month_picker(),
                            outcomes_map: outcomes_map_data,
                            on_toggle_month: move |_| show_month_picker.set(!show_month_picker()),
                            on_select_day: move |offset| {
                                selected_day_offset.set(offset);
                                refresh_timeline(offset, active_tag_filter.read().clone());
                            }
                        }

                        TimelineView {
                            snap: snap.clone(),
                            streak: streak_summary(),
                            note_draft: note_draft(),
                            active_tag: active_tag_filter(),
                            on_input_note: move |v| note_draft.set(v),
                            on_save_note: move |_| save_quick_note(()),
                            on_clear_tag: move |_| {
                                active_tag_filter.set(None);
                                refresh_timeline(selected_day_offset(), None);
                            },
                            on_select_tag: move |tag: String| {
                                active_tag_filter.set(Some(tag.clone()));
                                refresh_timeline(selected_day_offset(), Some(tag));
                            },
                            on_test_alarm: move |id: String| {
                                let test_trigger = current_epoch_ms() + 10_000;
                                match schedule_native_alarm(&id, test_trigger) {
                                    Ok(()) => status_msg.set(String::from("Alarme agendado para daqui a 10s.")),
                                    Err(err) => status_msg.set(format!("Falha JNI: {err}")),
                                }
                            },
                            on_delete_note: move |note_id: String| {
                                let db = resolve_db_path();
                                let now_ms = current_epoch_ms();
                                if let Ok(store) = CoreStore::open(db) {
                                    if store.delete_note(&note_id, now_ms).is_ok() {
                                        status_msg.set(String::from("Nota removida."));
                                        refresh_timeline(selected_day_offset(), active_tag_filter.read().clone());
                                    }
                                }
                            }
                        }
                    }
                }

                // TAB 2: HÁBITOS & ALARMES
                if active_tab() == ActiveTab::Habits {
                    HabitsView {
                        alarms: alarms(),
                        editing_id: editing_id(),
                        permission_blocked_n3: permission_blocked_n3(),
                        cooldown_blocked_id: cooldown_blocked_id(),
                        title: title(),
                        hour: hour(),
                        minute: minute(),
                        weekdays_mask: weekdays_mask(),
                        level: level(),
                        target_label: target_label(),
                        grace_window_minutes: grace_window_minutes(),
                        on_goto_permissions: move |_| {
                            health_state.set(check_native_permissions());
                            active_tab.set(ActiveTab::Health);
                        },
                        on_request_cooldown_unlock: move |blocked_id: String| {
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
                        on_save_alarm: move |_| save_alarm(()),
                        on_cancel_edit: move |_| reset_form(),
                        on_edit_alarm: move |item: AlarmSpec| {
                            editing_id.set(Some(item.id.clone()));
                            title.set(item.title.clone());
                            hour.set(item.hour);
                            minute.set(item.minute);
                            weekdays_mask.set(item.weekdays_mask);
                            level.set(item.level);
                            if !item.target_label.is_empty() {
                                target_label.set(item.target_label.clone());
                            }
                            grace_window_minutes.set(item.grace_window_minutes);
                            enabled.set(item.enabled);
                        },
                        on_toggle_enabled: move |(id, next_enabled, item_level): (String, bool, AlarmLevel)| {
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
                                match store.set_alarm_enabled(&id, next_enabled, now_ms) {
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
                                        cooldown_blocked_id.set(Some(id.clone()));
                                        status_msg.set(format!("Bloqueio 24h ativo até {unlock_at_epoch_ms}"));
                                    }
                                    Err(err) => status_msg.set(format!("Erro: {err}")),
                                }
                            }
                        },
                        on_test_alarm: move |id: String| {
                            let test_trigger = current_epoch_ms() + 10_000;
                            match schedule_native_alarm(&id, test_trigger) {
                                Ok(()) => status_msg.set(String::from("Teste agendado para +10s.")),
                                Err(err) => status_msg.set(format!("Erro JNI: {err}")),
                            }
                        },
                        on_delete_alarm: move |id: String| {
                            let db = resolve_db_path();
                            let now_ms = current_epoch_ms();
                            if let Ok(store) = CoreStore::open(db) {
                                match store.delete_alarm(&id, now_ms) {
                                    Ok(()) => {
                                        let _ = cancel_native_alarm(&id);
                                        status_msg.set(String::from("Hábito removido com sucesso."));
                                        reload_all(None);
                                    }
                                    Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                                        cooldown_blocked_id.set(Some(id.clone()));
                                        status_msg.set(format!("Bloqueio 24h ativo até {unlock_at_epoch_ms}"));
                                    }
                                    Err(err) => status_msg.set(format!("Erro ao remover: {err}")),
                                }
                            }
                        },
                        on_update_title: move |t: String| title.set(t),
                        on_update_hour: move |h: u8| hour.set(h),
                        on_update_minute: move |m: u8| minute.set(m),
                        on_update_grace: move |g: u16| grace_window_minutes.set(g),
                        on_toggle_weekday: move |bit: u8| weekdays_mask.set(weekdays_mask() ^ bit),
                        on_update_level: move |l: AlarmLevel| level.set(l),
                        on_update_target: move |t: String| target_label.set(t),
                    }
                }

                // TAB 3: PERMISSÕES DO SISTEMA
                if active_tab() == ActiveTab::Health {
                    HealthView {
                        current_health: current_health.clone(),
                        on_refresh: move |_| {
                            let refreshed = check_native_permissions();
                            if refreshed.all_granted() {
                                permission_blocked_n3.set(false);
                            }
                            health_state.set(refreshed);
                            status_msg.set(String::from("Permissões atualizadas."));
                        },
                        on_open_setting: move |key: &'static str| {
                            match open_native_permission_setting(key) {
                                Ok(()) => status_msg.set(format!("A abrir '{key}'...")),
                                Err(err) => status_msg.set(format!("Erro JNI: {err}")),
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
                        circle { cx: "12", cy: "12", r: "9" }
                        polyline { points: "12 7 12 12 15 15" }
                    }
                    span { "Hábitos" }
                }
                button {
                    class: if active_tab() == ActiveTab::Health { "nav-item nav-item-active" } else { "nav-item" },
                    onclick: move |_| {
                        active_tab.set(ActiveTab::Health);
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
                        path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
                    }
                    span { "Sistema" }
                }
            }
        }
    }
}

fn resolve_db_path() -> &'static str {
    DB_PATH_CACHE.get_or_init(|| {
        #[cfg(target_os = "android")]
        {
            if let Ok(dir) = android_files_dir() {
                return format!("{dir}/crabcheck.db");
            }
        }
        String::from("/data/data/com.crabcheck.CrabcheckApp/files/crabcheck.db")
    })
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(target_os = "android")]
fn clear_jni_exception(env: &mut jni::JNIEnv<'_>, err: impl std::fmt::Display) -> String {
    let msg = err.to_string();
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
    msg
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
        let fixed_epoch_ms = 1792756800000;
        let (header, days) = calculate_week_strip(fixed_epoch_ms, 0);
        assert!(!header.is_empty());
        assert_eq!(days.len(), 7);
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
