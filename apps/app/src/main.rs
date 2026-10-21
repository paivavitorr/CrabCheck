#![cfg_attr(not(target_os = "android"), allow(dead_code))]

use crabcheck_core::{AlarmLevel, AlarmSpec, CoreStore};
use dioxus::prelude::*;

// Ensure crabcheck-core-ffi symbols remain linked into the main binary on Android
#[used]
static KEEP_FFI: fn(String, String) -> Result<crabcheck_core_ffi::FfiAlarmDto, crabcheck_core_ffi::FfiError> =
    crabcheck_core_ffi::ffi_get_alarm;

const SPIKE_ALARM_ID: &str = "spike-001";

fn main() {
    #[cfg(target_os = "android")]
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let mut status_msg = use_signal(|| String::from("Pronto para testar o Spike Fase 0."));
    let mut outcomes = use_signal(Vec::<String>::new);

    let mut refresh_outcomes = move || {
        let db_path = resolve_db_path();
        match CoreStore::open(&db_path).and_then(|s| s.list_outcomes(SPIKE_ALARM_ID)) {
            Ok(list) => {
                let formatted = list
                    .into_iter()
                    .map(|r| format!("#{} [{}] @ {}", r.id, r.outcome, r.recorded_at_epoch_ms))
                    .collect();
                outcomes.set(formatted);
            }
            Err(err) => {
                status_msg.set(format!("Erro ao ler resultados: {err}"));
            }
        }
    };

    let trigger_spike_alarm = move |_| {
        let db_path = resolve_db_path();
        let now_ms = current_epoch_ms();
        let trigger_at = now_ms + 10_000;

        let spec = AlarmSpec {
            id: SPIKE_ALARM_ID.to_string(),
            title: "Ginásio às 06:00 (Spike)".to_string(),
            level: AlarmLevel::ProofOfAction,
            target_label: "Shoe".to_string(),
            trigger_at_epoch_ms: trigger_at,
        };

        let store_res = CoreStore::open(&db_path).and_then(|s| s.upsert_alarm(&spec));
        if let Err(err) = store_res {
            status_msg.set(format!("Erro ao gravar no SQLite: {err}"));
            return;
        }

        match schedule_native_alarm(&spec.id, spec.trigger_at_epoch_ms) {
            Ok(()) => {
                status_msg.set(format!(
                    "Alarme '{}' agendado para +10s ({trigger_at}). Bloqueie o ecrã!",
                    spec.id
                ));
                refresh_outcomes();
            }
            Err(err) => {
                status_msg.set(format!("SQLite gravado, falha no JNI: {err}"));
            }
        }
    };

    rsx! {
        div {
            style: "font-family: sans-serif; padding: 24px; max-width: 480px; margin: 0 auto;",
            h1 { "CrabCheck — Spike Fase 0" }
            p { "{status_msg}" }
            div {
                style: "display: flex; gap: 12px; margin: 16px 0;",
                button {
                    style: "padding: 12px 16px; background: #d81b60; color: white; border: none; border-radius: 6px; font-weight: bold;",
                    onclick: trigger_spike_alarm,
                    "Agendar Alarme (+10s)"
                }
                button {
                    style: "padding: 12px 16px; background: #008577; color: white; border: none; border-radius: 6px;",
                    onclick: move |_| refresh_outcomes(),
                    "Atualizar Resultados"
                }
            }
            h2 { "Resultados no SQLite ({SPIKE_ALARM_ID})" }
            ul {
                for item in outcomes() {
                    li { "{item}" }
                }
            }
        }
    }
}

fn current_epoch_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(0))
        .unwrap_or(0)
}

#[cfg(target_os = "android")]
fn resolve_db_path() -> String {
    android_files_dir()
        .map(|dir| format!("{dir}/crabcheck.db"))
        .unwrap_or_else(|_| String::from("/data/local/tmp/crabcheck.db"))
}

#[cfg(not(target_os = "android"))]
fn resolve_db_path() -> String {
    std::env::temp_dir()
        .join("crabcheck_spike.db")
        .to_string_lossy()
        .into_owned()
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
fn schedule_native_alarm(alarm_id: &str, trigger_at_epoch_ms: i64) -> Result<(), String> {
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
    let alarm_id_jstr = env
        .new_string(alarm_id)
        .map_err(|e| clear_jni_exception(&mut env, e))?;

    env.call_static_method(
        scheduler_class,
        "scheduleExactAlarm",
        "(Landroid/content/Context;Ljava/lang/String;J)V",
        &[
            JValue::Object(&context_obj),
            JValue::Object(&alarm_id_jstr),
            JValue::Long(trigger_at_epoch_ms),
        ],
    )
    .map_err(|e| clear_jni_exception(&mut env, e))?;

    Ok(())
}

#[cfg(not(target_os = "android"))]
fn schedule_native_alarm(_alarm_id: &str, _trigger_at_epoch_ms: i64) -> Result<(), String> {
    Ok(())
}
