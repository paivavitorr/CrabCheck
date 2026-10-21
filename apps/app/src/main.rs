#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use std::sync::OnceLock;

use crabcheck_core::{
    AlarmDraft, AlarmLevel, AlarmOutcomeRecord, AlarmSpec, CoreError, CoreStore,
    CURATED_OBJECT_LABELS,
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

fn main() {
    #[cfg(target_os = "android")]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut alarms = use_signal(|| {
        let db = resolve_db_path();
        CoreStore::open(db)
            .and_then(|s| s.list_alarms())
            .unwrap_or_default()
    });
    let mut selected_alarm_id = use_signal(|| Option::<String>::None);
    let mut outcomes = use_signal(Vec::<AlarmOutcomeRecord>::new);
    let mut status_msg = use_signal(|| String::from("Gerencie os seus alarmes e hábitos."));
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

    let mut reload_all = move |maybe_select_id: Option<String>| {
        let db = resolve_db_path();
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

        let spec = match AlarmSpec::new_from_draft(draft, now_ms, existing.as_ref()) {
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
                            "Alarme '{}' ({:02}:{:02}) guardado com sucesso.",
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
                status_msg.set(format!("Erro ao guardar alarme: {err}"));
            }
        }
    };

    rsx! {
        div {
            style: "font-family: sans-serif; padding: 16px; max-width: 540px; margin: 0 auto; color: #1f2937;",
            h1 { style: "margin-bottom: 4px;", "CrabCheck — Hábitos & Alarmes" }
            p {
                style: "background: #f3f4f6; padding: 10px; border-radius: 6px; font-size: 14px;",
                "{status_msg}"
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
                    if editing_id().is_some() { "Editar Alarme" } else { "Novo Alarme" }
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
                        if editing_id().is_some() { "Atualizar Alarme" } else { "Criar Alarme" }
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
            h2 { style: "font-size: 18px; margin-bottom: 10px;", "Alarmes Registados" }
            if alarms().is_empty() {
                p { style: "color: #6b7280; font-size: 14px;", "Nenhum alarme configurado ainda." }
            }
            for item in alarms() {
                {
                    let id_for_toggle = item.id.clone();
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
                                                    status_msg.set(String::from("Alarme excluído."));
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
                        p { style: "font-size: 13px; color: #6b7280;", "Sem registos para este alarme." }
                    }
                    ul {
                        for out in outcomes() {
                            li {
                                key: "{out.id}",
                                style: "font-size: 13px; margin-bottom: 6px;",
                                div {
                                    "#{out.id} [{out.outcome}] @ {out.recorded_at_epoch_ms}"
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
fn with_alarm_scheduler_class<F>(f: F) -> Result<(), String>
where
    F: FnOnce(
        &mut jni::JNIEnv<'_>,
        &jni::objects::JObject<'_>,
        jni::objects::JClass<'_>,
    ) -> Result<(), String>,
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

#[cfg(not(target_os = "android"))]
fn schedule_native_alarm(_alarm_id: &str, _trigger_at_epoch_ms: i64) -> Result<(), String> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
fn cancel_native_alarm(_alarm_id: &str) -> Result<(), String> {
    Ok(())
}
