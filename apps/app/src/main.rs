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

const WEEKDAY_NAMES: &[(&str, u8)] = &[
    ("Seg", 1 << 0),
    ("Ter", 1 << 1),
    ("Qua", 1 << 2),
    ("Qui", 1 << 3),
    ("Sex", 1 << 4),
    ("Sáb", 1 << 5),
    ("Dom", 1 << 6),
];

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
    let mut status_msg =
        use_signal(|| String::from("Bem-vindo ao CrabCheck — Agenda e Prova de Ação."));
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

    rsx! {
        div {
            style: "font-family: sans-serif; padding: 14px; max-width: 560px; margin: 0 auto; color: #1f2937;",

            // Header
            div {
                style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;",
                h1 { style: "margin: 0; font-size: 22px;", "CrabCheck" }
                span {
                    style: if current_health.all_granted() {
                        "font-size: 12px; padding: 4px 8px; background: #d1fae5; color: #065f46; border-radius: 12px; font-weight: bold;"
                    } else {
                        "font-size: 12px; padding: 4px 8px; background: #fee2e2; color: #991b1b; border-radius: 12px; font-weight: bold;"
                    },
                    "Saúde: {granted_count}/6"
                }
            }

            // Status bar
            p {
                style: "background: #f3f4f6; padding: 10px; border-radius: 6px; font-size: 13px; margin: 0 0 12px 0;",
                "{status_msg}"
            }

            // Global Onboarding Alert if any critical permission is missing
            if !current_health.all_granted() && active_tab() != ActiveTab::Health {
                div {
                    style: "background: #fffbeb; border: 1px solid #f59e0b; padding: 10px; border-radius: 6px; margin-bottom: 12px; display: flex; justify-content: space-between; align-items: center; gap: 8px;",
                    div {
                        style: "font-size: 12px; color: #92400e;",
                        strong { "Onboarding Pendente ({granted_count}/6): " }
                        "Conceda todas as permissões críticas para criar ou ativar hábitos Nível 3."
                    }
                    button {
                        style: "padding: 6px 10px; background: #d97706; color: white; border: none; border-radius: 4px; font-size: 12px; font-weight: bold; white-space: nowrap;",
                        onclick: move |_| {
                            health_state.set(check_native_permissions());
                            active_tab.set(ActiveTab::Health);
                        },
                        "Configurar"
                    }
                }
            }

            // Navigation Tabs
            div {
                style: "display: flex; gap: 6px; margin-bottom: 16px; border-bottom: 2px solid #e5e7eb; padding-bottom: 8px;",
                button {
                    style: if active_tab() == ActiveTab::Timeline {
                        "flex: 1; padding: 9px; background: #111827; color: white; border: none; border-radius: 6px; font-weight: bold; font-size: 13px;"
                    } else {
                        "flex: 1; padding: 9px; background: #f3f4f6; color: #374151; border: none; border-radius: 6px; font-size: 13px;"
                    },
                    onclick: move |_| {
                        active_tab.set(ActiveTab::Timeline);
                        reload_all(None);
                    },
                    "Timeline Diária"
                }
                button {
                    style: if active_tab() == ActiveTab::Habits {
                        "flex: 1; padding: 9px; background: #111827; color: white; border: none; border-radius: 6px; font-weight: bold; font-size: 13px;"
                    } else {
                        "flex: 1; padding: 9px; background: #f3f4f6; color: #374151; border: none; border-radius: 6px; font-size: 13px;"
                    },
                    onclick: move |_| {
                        active_tab.set(ActiveTab::Habits);
                        reload_all(None);
                    },
                    "Hábitos ({alarms().len()})"
                }
                button {
                    style: if active_tab() == ActiveTab::Health {
                        "flex: 1; padding: 9px; background: #111827; color: white; border: none; border-radius: 6px; font-weight: bold; font-size: 13px;"
                    } else {
                        "flex: 1; padding: 9px; background: #f3f4f6; color: #374151; border: none; border-radius: 6px; font-size: 13px;"
                    },
                    onclick: move |_| {
                        health_state.set(check_native_permissions());
                        active_tab.set(ActiveTab::Health);
                    },
                    "Permissões ({granted_count}/6)"
                }
            }

            // TAB 1: TIMELINE DIÁRIA
            if active_tab() == ActiveTab::Timeline {
                div {
                    // Date Navigation Bar
                    div {
                        style: "display: flex; justify-content: space-between; align-items: center; background: #f9fafb; border: 1px solid #e5e7eb; border-radius: 8px; padding: 8px 12px; margin-bottom: 14px;",
                        button {
                            style: "padding: 6px 10px; background: #e5e7eb; color: #1f2937; border: none; border-radius: 4px; font-size: 12px; font-weight: bold;",
                            onclick: move |_| {
                                let next_offset = selected_day_offset() - 1;
                                selected_day_offset.set(next_offset);
                                refresh_timeline(next_offset, active_tag_filter.read().clone());
                            },
                            "◀ Anterior"
                        }
                        div {
                            style: "text-align: center;",
                            strong { style: "font-size: 15px;", "{snap.date_iso}" }
                            if selected_day_offset() == 0 {
                                span {
                                    style: "margin-left: 6px; font-size: 11px; padding: 2px 6px; background: #dbeafe; color: #1e40af; border-radius: 4px;",
                                    "Hoje"
                                }
                            } else {
                                button {
                                    style: "margin-left: 8px; padding: 2px 8px; background: #2563eb; color: white; border: none; border-radius: 4px; font-size: 11px;",
                                    onclick: move |_| {
                                        selected_day_offset.set(0);
                                        refresh_timeline(0, active_tag_filter.read().clone());
                                    },
                                    "Ir para Hoje"
                                }
                            }
                        }
                        button {
                            style: "padding: 6px 10px; background: #e5e7eb; color: #1f2937; border: none; border-radius: 4px; font-size: 12px; font-weight: bold;",
                            onclick: move |_| {
                                let next_offset = selected_day_offset() + 1;
                                selected_day_offset.set(next_offset);
                                refresh_timeline(next_offset, active_tag_filter.read().clone());
                            },
                            "Seguinte ▶"
                        }
                    }

                    // Quick Note Composer (Memos-style)
                    div {
                        style: "border: 1px solid #d1d5db; border-radius: 8px; padding: 12px; margin-bottom: 16px; background: #ffffff;",
                        label {
                            style: "display: block; font-size: 13px; font-weight: bold; margin-bottom: 6px;",
                            "Nova Nota Rápida (use #tags para organizar)"
                        }
                        div {
                            style: "display: flex; gap: 8px;",
                            input {
                                style: "flex: 1; padding: 8px; border: 1px solid #d1d5db; border-radius: 6px; font-size: 13px;",
                                r#type: "text",
                                placeholder: "Ex.: Treino concluído sem falhas #foco #energia",
                                value: "{note_draft}",
                                oninput: move |evt| note_draft.set(evt.value())
                            }
                            button {
                                style: "padding: 8px 14px; background: #008577; color: white; border: none; border-radius: 6px; font-weight: bold; font-size: 13px;",
                                onclick: save_quick_note,
                                "Guardar Nota"
                            }
                        }
                    }

                    // Active #tag filter indicator
                    if let Some(tag) = active_tag_filter() {
                        div {
                            style: "display: flex; justify-content: space-between; align-items: center; background: #eff6ff; border: 1px solid #93c5fd; padding: 8px 12px; border-radius: 6px; margin-bottom: 14px;",
                            span {
                                style: "font-size: 13px; color: #1e3a8a;",
                                "A filtrar notas pela tag: "
                                strong { "#{tag}" }
                            }
                            button {
                                style: "padding: 4px 8px; background: #1d4ed8; color: white; border: none; border-radius: 4px; font-size: 11px;",
                                onclick: move |_| {
                                    active_tag_filter.set(None);
                                    refresh_timeline(selected_day_offset(), None);
                                },
                                "Limpar Filtro"
                            }
                        }
                    }

                    // Section 1: Scheduled Habits for this Day
                    h2 { style: "font-size: 16px; margin: 0 0 8px 0;", "Hábitos Agendados do Dia ({snap.scheduled_alarms.len()})" }
                    if snap.scheduled_alarms.is_empty() {
                        p { style: "font-size: 13px; color: #6b7280; margin-bottom: 16px;", "Nenhum hábito agendado para esta data." }
                    }
                    for item in snap.scheduled_alarms.iter() {
                        {
                            let id_for_test = item.id.clone();
                            rsx! {
                                div {
                                    key: "sched-{item.id}",
                                    style: "border-left: 4px solid #2563eb; background: #f9fafb; padding: 10px 12px; border-radius: 6px; margin-bottom: 8px; display: flex; justify-content: space-between; align-items: center;",
                                    div {
                                        strong { style: "font-size: 14px;", "{item.hour:02}:{item.minute:02} — {item.title}" }
                                        div {
                                            style: "font-size: 12px; color: #4b5563; margin-top: 2px;",
                                            "Nível {item.level.as_i32()} | Janela: {item.grace_window_minutes}m"
                                            if item.level == AlarmLevel::ProofOfAction {
                                                " | Prova: {item.target_label}"
                                            }
                                        }
                                    }
                                    button {
                                        style: "padding: 5px 8px; background: #d81b60; color: white; border: none; border-radius: 4px; font-size: 11px;",
                                        onclick: move |_| {
                                            let test_trigger = current_epoch_ms() + 10_000;
                                            match schedule_native_alarm(&id_for_test, test_trigger) {
                                                Ok(()) => status_msg.set(format!("Disparo de teste agendado para +10s ({id_for_test}).")),
                                                Err(err) => status_msg.set(format!("Falha JNI: {err}")),
                                            }
                                        },
                                        "Testar +10s"
                                    }
                                }
                            }
                        }
                    }

                    // Section 2: Recorded Outcomes & Visual Proofs of the Day
                    h2 { style: "font-size: 16px; margin: 16px 0 8px 0;", "Resultados e Provas do Dia ({snap.outcomes.len()})" }
                    if snap.outcomes.is_empty() {
                        p { style: "font-size: 13px; color: #6b7280; margin-bottom: 16px;", "Sem resultados registados nesta data." }
                    }
                    for out in snap.outcomes.iter() {
                        {
                            let time_str = format_local_time_hms(out.recorded_at_epoch_ms);
                            let badge_style = match out.outcome.as_str() {
                                "CUMPRIDO" => "padding: 2px 6px; background: #d1fae5; color: #065f46; border-radius: 4px; font-weight: bold; font-size: 11px;",
                                "FALHOU" => "padding: 2px 6px; background: #fee2e2; color: #991b1b; border-radius: 4px; font-weight: bold; font-size: 11px;",
                                "ADIADO" => "padding: 2px 6px; background: #fef3c7; color: #92400e; border-radius: 4px; font-weight: bold; font-size: 11px;",
                                _ => "padding: 2px 6px; background: #e5e7eb; color: #374151; border-radius: 4px; font-weight: bold; font-size: 11px;",
                            };
                            rsx! {
                                div {
                                    key: "tl-out-{out.id}",
                                    style: "border: 1px solid #e5e7eb; border-radius: 6px; padding: 10px 12px; margin-bottom: 8px; background: #ffffff;",
                                    div {
                                        style: "display: flex; justify-content: space-between; align-items: center;",
                                        strong { style: "font-size: 13px;", "{time_str} — {out.alarm_title} (N{out.alarm_level.as_i32()})" }
                                        span { style: "{badge_style}", "{out.outcome}" }
                                    }
                                    if let Some(lbl) = &out.matched_label {
                                        div {
                                            style: "font-size: 12px; color: #374151; margin-top: 4px;",
                                            if let Some(conf) = out.confidence {
                                                "Reconhecimento ML Kit: {lbl} ({conf * 100.0:.0}%)"
                                            } else {
                                                "Reconhecimento ML Kit: {lbl}"
                                            }
                                        }
                                    }
                                    if let Some(path) = &out.proof_image_path {
                                        div {
                                            style: "font-size: 11px; color: #047857; margin-top: 4px; word-break: break-all;",
                                            "📷 Prova gravada: {path}"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Section 3: Quick Notes of the Day
                    h2 { style: "font-size: 16px; margin: 16px 0 8px 0;", "Notas Rápidas ({snap.notes.len()})" }
                    if snap.notes.is_empty() {
                        p { style: "font-size: 13px; color: #6b7280;", "Nenhuma nota rápida registada nesta data." }
                    }
                    for note in snap.notes.iter() {
                        {
                            let note_id = note.id.clone();
                            let time_str = format_local_time_hm(note.created_at_epoch_ms);
                            rsx! {
                                div {
                                    key: "note-{note.id}",
                                    style: "border: 1px solid #e5e7eb; border-radius: 6px; padding: 10px 12px; margin-bottom: 8px; background: #fffbeb;",
                                    div {
                                        style: "display: flex; justify-content: space-between; align-items: center; margin-bottom: 4px;",
                                        span { style: "font-size: 11px; color: #6b7280;", "🕒 {time_str}" }
                                        button {
                                            style: "padding: 2px 6px; background: #ef4444; color: white; border: none; border-radius: 4px; font-size: 11px;",
                                            onclick: move |_| {
                                                let db = resolve_db_path();
                                                let now_ms = current_epoch_ms();
                                                if let Ok(store) = CoreStore::open(db) {
                                                    if store.delete_note(&note_id, now_ms).is_ok() {
                                                        status_msg.set(String::from("Nota excluída."));
                                                        refresh_timeline(selected_day_offset(), active_tag_filter.read().clone());
                                                    }
                                                }
                                            },
                                            "Excluir"
                                        }
                                    }
                                    div {
                                        style: "font-size: 14px; color: #111827; margin-bottom: 6px;",
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
                                                            style: "padding: 2px 8px; background: #dbeafe; color: #1e40af; border: none; border-radius: 10px; font-size: 11px;",
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

            // TAB 2: HÁBITOS & ALARMES
            if active_tab() == ActiveTab::Habits {
                div {
                    if permission_blocked_n3() {
                        div {
                            style: "background: #fee2e2; border: 1px solid #ef4444; padding: 10px; border-radius: 6px; margin-bottom: 12px;",
                            p {
                                style: "margin: 0 0 8px 0; font-size: 13px; color: #991b1b;",
                                "Não é possível criar ou ativar um hábito Nível 3 enquanto houver permissões críticas pendentes."
                            }
                            button {
                                style: "padding: 8px 12px; background: #dc2626; color: white; border: none; border-radius: 4px; font-weight: bold; font-size: 12px;",
                                onclick: move |_| {
                                    health_state.set(check_native_permissions());
                                    active_tab.set(ActiveTab::Health);
                                },
                                "Abrir Onboarding de Permissões"
                            }
                        }
                    }

                    if let Some(blocked_id) = cooldown_blocked_id() {
                        div {
                            style: "background: #fef3c7; border: 1px solid #f59e0b; padding: 10px; border-radius: 6px; margin-bottom: 12px;",
                            p { style: "margin: 0 0 8px 0; font-size: 13px;", "Este hábito Nível 3 está protegido contra desativação ou alívio imediato." }
                            button {
                                style: "padding: 8px 12px; background: #d97706; color: white; border: none; border-radius: 4px; font-weight: bold;",
                                onclick: move |_| {
                                    let db = resolve_db_path();
                                    let now_ms = current_epoch_ms();
                                    if let Ok(store) = CoreStore::open(db) {
                                        match store.request_cooldown_unlock(&blocked_id, now_ms) {
                                            Ok(unlock_at) => {
                                                status_msg.set(format!("Pedido de desbloqueio registado! Liberado após 24h (epoch {unlock_at})."));
                                                cooldown_blocked_id.set(None);
                                                reload_all(None);
                                            }
                                            Err(err) => status_msg.set(format!("Erro ao solicitar desbloqueio: {err}")),
                                        }
                                    }
                                },
                                "Solicitar Desbloqueio (24h)"
                            }
                        }
                    }

                    // Form Card
                    div {
                        style: "border: 1px solid #e5e7eb; border-radius: 8px; padding: 14px; margin-bottom: 20px;",
                        h2 {
                            style: "margin-top: 0; font-size: 18px;",
                            if editing_id().is_some() { "Editar Hábito" } else { "Novo Hábito" }
                        }

                        div {
                            style: "margin-bottom: 10px;",
                            label { style: "display: block; font-size: 13px; font-weight: bold;", "Título do Hábito" }
                            input {
                                style: "width: 100%; padding: 8px; box-sizing: border-box; margin-top: 4px;",
                                r#type: "text",
                                value: "{title}",
                                oninput: move |evt| title.set(evt.value())
                            }
                        }

                        div {
                            style: "display: flex; gap: 12px; margin-bottom: 10px;",
                            div {
                                style: "flex: 1;",
                                label { style: "display: block; font-size: 13px; font-weight: bold;", "Hora (0-23)" }
                                input {
                                    style: "width: 100%; padding: 8px; box-sizing: border-box; margin-top: 4px;",
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
                                label { style: "display: block; font-size: 13px; font-weight: bold;", "Minuto (0-59)" }
                                input {
                                    style: "width: 100%; padding: 8px; box-sizing: border-box; margin-top: 4px;",
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
                                label { style: "display: block; font-size: 13px; font-weight: bold;", "Janela (min)" }
                                input {
                                    style: "width: 100%; padding: 8px; box-sizing: border-box; margin-top: 4px;",
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
                            style: "margin-bottom: 10px;",
                            label { style: "display: block; font-size: 13px; font-weight: bold; margin-bottom: 4px;", "Dias da Semana (nenhum = Evento Único)" }
                            div {
                                style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                for (name, bit) in WEEKDAY_NAMES.iter().copied() {
                                    button {
                                        style: if (weekdays_mask() & bit) != 0 {
                                            "padding: 6px 10px; background: #2563eb; color: white; border: none; border-radius: 4px; font-size: 12px;"
                                        } else {
                                            "padding: 6px 10px; background: #e5e7eb; color: #374151; border: none; border-radius: 4px; font-size: 12px;"
                                        },
                                        onclick: move |_| weekdays_mask.set(weekdays_mask() ^ bit),
                                        "{name}"
                                    }
                                }
                            }
                        }

                        div {
                            style: "margin-bottom: 10px;",
                            label { style: "display: block; font-size: 13px; font-weight: bold; margin-bottom: 4px;", "Nível de Atrito" }
                            div {
                                style: "display: flex; gap: 8px;",
                                for (lvl, label_txt) in [
                                    (AlarmLevel::Passive, "N1 Passivo"),
                                    (AlarmLevel::Persistent, "N2 Persistente"),
                                    (AlarmLevel::ProofOfAction, "N3 Prova IA"),
                                ] {
                                    button {
                                        style: if level() == lvl {
                                            "flex: 1; padding: 8px; background: #d81b60; color: white; border: none; border-radius: 4px; font-weight: bold;"
                                        } else {
                                            "flex: 1; padding: 8px; background: #e5e7eb; color: #374151; border: none; border-radius: 4px;"
                                        },
                                        onclick: move |_| level.set(lvl),
                                        "{label_txt}"
                                    }
                                }
                            }
                        }

                        if level() == AlarmLevel::ProofOfAction {
                            div {
                                style: "margin-bottom: 12px;",
                                label { style: "display: block; font-size: 13px; font-weight: bold; margin-bottom: 4px;", "Objeto de Prova (Catálogo ML Kit)" }
                                div {
                                    style: "display: flex; gap: 6px; flex-wrap: wrap;",
                                    for (code, friendly) in CURATED_OBJECT_LABELS.iter().copied() {
                                        button {
                                            style: if target_label() == code {
                                                "padding: 6px 10px; background: #008577; color: white; border: none; border-radius: 4px; font-size: 12px;"
                                            } else {
                                                "padding: 6px 10px; background: #e5e7eb; color: #374151; border: none; border-radius: 4px; font-size: 12px;"
                                            },
                                            onclick: move |_| target_label.set(code.to_string()),
                                            "{friendly} ({code})"
                                        }
                                    }
                                }
                            }
                        }

                        div {
                            style: "display: flex; gap: 10px;",
                            button {
                                style: "flex: 1; padding: 10px; background: #111827; color: white; border: none; border-radius: 6px; font-weight: bold;",
                                onclick: save_alarm,
                                if editing_id().is_some() { "Atualizar Hábito" } else { "Criar Hábito" }
                            }
                            if editing_id().is_some() {
                                button {
                                    style: "padding: 10px 14px; background: #9ca3af; color: white; border: none; border-radius: 6px;",
                                    onclick: move |_| reset_form(),
                                    "Cancelar"
                                }
                            }
                        }
                    }

                    // Alarms list
                    h2 { style: "font-size: 18px; margin-bottom: 10px;", "Hábitos Registados" }
                    if alarms().is_empty() {
                        p { style: "color: #6b7280; font-size: 14px;", "Nenhum hábito configurado ainda." }
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
                                    style: "border: 1px solid #d1d5db; border-radius: 8px; padding: 12px; margin-bottom: 10px;",
                                    div {
                                        style: "display: flex; justify-content: space-between; align-items: center;",
                                        div {
                                            strong { style: "font-size: 16px;", "{item.hour:02}:{item.minute:02} — {item.title}" }
                                            div {
                                                style: "font-size: 12px; color: #4b5563; margin-top: 4px;",
                                                "Nível {item.level.as_i32()} | {days_str} | Janela: {item.grace_window_minutes}m"
                                                if item.level == AlarmLevel::ProofOfAction {
                                                    " | Prova: {item.target_label}"
                                                }
                                            }
                                            if let Some(unlock_ts) = item.cooldown_unlock_at_epoch_ms {
                                                div {
                                                    style: "font-size: 11px; color: #b45309; margin-top: 2px;",
                                                    "Desbloqueio N3 agendado para: {unlock_ts}"
                                                }
                                            }
                                        }
                                        span {
                                            style: if item.enabled {
                                                "font-size: 12px; padding: 4px 8px; background: #d1fae5; color: #065f46; border-radius: 4px;"
                                            } else {
                                                "font-size: 12px; padding: 4px 8px; background: #f3f4f6; color: #6b7280; border-radius: 4px;"
                                            },
                                            if item.enabled { "Ativo" } else { "Inativo" }
                                        }
                                    }

                                    div {
                                        style: "display: flex; gap: 6px; flex-wrap: wrap; margin-top: 10px;",
                                        button {
                                            style: "padding: 6px 10px; background: #374151; color: white; border: none; border-radius: 4px; font-size: 12px;",
                                            onclick: move |_| {
                                                // R1: Only block when activating an N3 habit without permissions
                                                if next_enabled && item_level == AlarmLevel::ProofOfAction {
                                                    let h = check_native_permissions();
                                                    health_state.set(h.clone());
                                                    if !h.all_granted() {
                                                        permission_blocked_n3.set(true);
                                                        status_msg.set(format!(
                                                            "Permissões pendentes para ativar Nível 3: {}.",
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
                                                            status_msg.set(format!("Estado de '{}' alterado.", updated.title));
                                                            reload_all(Some(updated.id));
                                                        }
                                                        Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                                                            cooldown_blocked_id.set(Some(id_for_toggle.clone()));
                                                            status_msg.set(format!("Cooldown N3 ativo até {unlock_at_epoch_ms}"));
                                                        }
                                                        Err(err) => status_msg.set(format!("Erro: {err}")),
                                                    }
                                                }
                                            },
                                            if item.enabled { "Desativar" } else { "Ativar" }
                                        }

                                        button {
                                            style: "padding: 6px 10px; background: #2563eb; color: white; border: none; border-radius: 4px; font-size: 12px;",
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
                                            style: "padding: 6px 10px; background: #dc2626; color: white; border: none; border-radius: 4px; font-size: 12px;",
                                            onclick: move |_| {
                                                let db = resolve_db_path();
                                                let now_ms = current_epoch_ms();
                                                if let Ok(store) = CoreStore::open(db) {
                                                    match store.delete_alarm(&id_for_delete, now_ms) {
                                                        Ok(()) => {
                                                            let _ = cancel_native_alarm(&id_for_delete);
                                                            status_msg.set(String::from("Hábito excluído."));
                                                            reload_all(None);
                                                        }
                                                        Err(CoreError::CooldownActive { unlock_at_epoch_ms }) => {
                                                            cooldown_blocked_id.set(Some(id_for_delete.clone()));
                                                            status_msg.set(format!("Cooldown N3 ativo até {unlock_at_epoch_ms}"));
                                                        }
                                                        Err(err) => status_msg.set(format!("Erro ao excluir: {err}")),
                                                    }
                                                }
                                            },
                                            "Excluir"
                                        }

                                        button {
                                            style: "padding: 6px 10px; background: #d81b60; color: white; border: none; border-radius: 4px; font-size: 12px;",
                                            onclick: move |_| {
                                                let test_trigger = current_epoch_ms() + 10_000;
                                                match schedule_native_alarm(&id_for_test, test_trigger) {
                                                    Ok(()) => status_msg.set(format!("Disparo de teste agendado para +10s ({id_for_test}). Bloqueie o ecrã!")),
                                                    Err(err) => status_msg.set(format!("Falha JNI ao agendar teste: {err}")),
                                                }
                                            },
                                            "Testar +10s"
                                        }

                                        button {
                                            style: "padding: 6px 10px; background: #008577; color: white; border: none; border-radius: 4px; font-size: 12px;",
                                            onclick: move |_| reload_all(Some(id_for_outcomes.clone())),
                                            "Ver Histórico"
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if let Some(sel_id) = selected_alarm_id() {
                        div {
                            style: "margin-top: 20px; border-top: 1px solid #e5e7eb; padding-top: 12px;",
                            h3 { style: "font-size: 16px;", "Histórico de Resultados ({sel_id})" }
                            if outcomes().is_empty() {
                                p { style: "font-size: 13px; color: #6b7280;", "Sem registos para este hábito." }
                            }
                            ul {
                                for out in outcomes() {
                                    li {
                                        key: "{out.id}",
                                        style: "font-size: 13px; margin-bottom: 6px;",
                                        div {
                                            "#{out.id} [{out.outcome}] @ {format_local_time_hms(out.recorded_at_epoch_ms)}"
                                            if let Some(lbl) = &out.matched_label {
                                                if let Some(conf) = out.confidence {
                                                    " — ML Kit: {lbl} ({conf * 100.0:.0}%)"
                                                } else {
                                                    " — ML Kit: {lbl}"
                                                }
                                            }
                                        }
                                        if let Some(path) = &out.proof_image_path {
                                            div {
                                                style: "font-size: 11px; color: #047857;",
                                                "📷 Prova gravada: {path}"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // TAB 3: SAÚDE & ONBOARDING DE PERMISSÕES
            if active_tab() == ActiveTab::Health {
                div {
                    div {
                        style: if current_health.all_granted() {
                            "background: #ecfdf5; border: 1px solid #10b981; padding: 12px; border-radius: 8px; margin-bottom: 16px;"
                        } else {
                            "background: #fef2f2; border: 1px solid #ef4444; padding: 12px; border-radius: 8px; margin-bottom: 16px;"
                        },
                        div {
                            style: "display: flex; justify-content: space-between; align-items: center;",
                            div {
                                strong {
                                    style: "font-size: 15px;",
                                    if current_health.all_granted() {
                                        "Sistema 100% Pronto ({granted_count}/6 Permissões Ativas)"
                                    } else {
                                        "Onboarding de Permissões ({granted_count}/6 Ativas)"
                                    }
                                }
                                p {
                                    style: "margin: 4px 0 0 0; font-size: 12px; color: #374151;",
                                    "Hábitos Nível 3 exigem todas as 6 permissões abaixo para garantir bloqueio duro e validação por câmara."
                                }
                            }
                            button {
                                style: "padding: 8px 12px; background: #111827; color: white; border: none; border-radius: 6px; font-size: 12px; font-weight: bold;",
                                onclick: move |_| {
                                    let refreshed = check_native_permissions();
                                    if refreshed.all_granted() {
                                        permission_blocked_n3.set(false);
                                    }
                                    health_state.set(refreshed);
                                    status_msg.set(String::from("Estado das permissões revalidado."));
                                },
                                "Revalidar Agora"
                            }
                        }
                    }

                    for (key, name, desc, is_ok) in [
                        (
                            "exact_alarm",
                            "1. Alarme Exato (SCHEDULE_EXACT_ALARM)",
                            "Garante que o AlarmManager dispare no minuto exato mesmo em Doze mode.",
                            current_health.exact_alarm,
                        ),
                        (
                            "overlay",
                            "2. Sobreposição de Ecrã (SYSTEM_ALERT_WINDOW)",
                            "Permite abrir a LockActivity sobre outras janelas quando o alarme dispara.",
                            current_health.overlay,
                        ),
                        (
                            "accessibility",
                            "3. Serviço de Acessibilidade CrabCheck",
                            "Mantém o bloqueio Nível 3 ativo impedindo saída pelo botão Home ou Recentes.",
                            current_health.accessibility,
                        ),
                        (
                            "camera",
                            "4. Câmara (Prova de Ação ML Kit)",
                            "Necessária para validar o objeto exigido em tempo real com o ML Kit.",
                            current_health.camera,
                        ),
                        (
                            "notifications",
                            "5. Notificações e Full-Screen Intent",
                            "Exibe o serviço de áudio em primeiro plano e acorda o ecrã bloqueado.",
                            current_health.notifications,
                        ),
                        (
                            "battery",
                            "6. Isenção de Otimização de Bateria",
                            "Impede que o sistema suspenda o serviço sonoro ou o reagendamento.",
                            current_health.battery,
                        ),
                    ] {
                        div {
                            key: "perm-{key}",
                            style: "border: 1px solid #d1d5db; border-radius: 8px; padding: 12px; margin-bottom: 10px; display: flex; justify-content: space-between; align-items: center; gap: 10px;",
                            div {
                                strong { style: "font-size: 14px;", "{name}" }
                                div {
                                    style: "font-size: 12px; color: #4b5563; margin-top: 3px;",
                                    "{desc}"
                                }
                            }
                            div {
                                style: "display: flex; flex-direction: column; align-items: flex-end; gap: 6px;",
                                span {
                                    style: if is_ok {
                                        "font-size: 11px; padding: 3px 8px; background: #d1fae5; color: #065f46; border-radius: 4px; font-weight: bold;"
                                    } else {
                                        "font-size: 11px; padding: 3px 8px; background: #fee2e2; color: #991b1b; border-radius: 4px; font-weight: bold;"
                                    },
                                    if is_ok { "Ativo" } else { "Pendente" }
                                }
                                button {
                                    style: "padding: 6px 10px; background: #2563eb; color: white; border: none; border-radius: 4px; font-size: 12px; white-space: nowrap;",
                                    onclick: move |_| {
                                        match open_native_permission_setting(key) {
                                            Ok(()) => status_msg.set(format!("A abrir configuração para '{name}'... Clique em Revalidar ao voltar.")),
                                            Err(err) => status_msg.set(format!("Erro JNI ao abrir configuração: {err}")),
                                        }
                                    },
                                    if is_ok { "Rever" } else { "Configurar" }
                                }
                            }
                        }
                    }
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
