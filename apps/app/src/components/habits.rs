use crabcheck_core::{AlarmLevel, AlarmSpec, CURATED_OBJECT_LABELS};
use dioxus::prelude::*;

pub const WEEKDAY_NAMES: &[(&str, u8)] = &[
    ("Seg", 1 << 0),
    ("Ter", 1 << 1),
    ("Qua", 1 << 2),
    ("Qui", 1 << 3),
    ("Sex", 1 << 4),
    ("Sáb", 1 << 5),
    ("Dom", 1 << 6),
];

pub fn format_weekdays(mask: u8) -> String {
    if mask == 0 {
        return "Uma vez".to_string();
    }
    if mask == 0b0111_1111 {
        return "Todos os dias".to_string();
    }
    if mask == 0b0001_1111 {
        return "Seg a Sex".to_string();
    }
    if mask == 0b0110_0000 {
        return "Fins de semana".to_string();
    }
    let mut names = Vec::new();
    for (name, bit) in WEEKDAY_NAMES.iter().copied() {
        if (mask & bit) != 0 {
            names.push(name);
        }
    }
    names.join(", ")
}

#[component]
pub fn HabitsView(
    alarms: Vec<AlarmSpec>,
    editing_id: Option<String>,
    permission_blocked_n3: bool,
    cooldown_blocked_id: Option<String>,
    title: String,
    hour: u8,
    minute: u8,
    weekdays_mask: u8,
    level: AlarmLevel,
    target_label: String,
    grace_window_minutes: u16,
    on_goto_permissions: EventHandler<()>,
    on_request_cooldown_unlock: EventHandler<String>,
    on_force_delete_alarm: EventHandler<String>,
    on_save_alarm: EventHandler<()>,
    on_cancel_edit: EventHandler<()>,
    on_edit_alarm: EventHandler<AlarmSpec>,
    on_toggle_enabled: EventHandler<(String, bool, AlarmLevel)>,
    on_test_alarm: EventHandler<String>,
    on_delete_alarm: EventHandler<String>,
    on_update_title: EventHandler<String>,
    on_update_hour: EventHandler<u8>,
    on_update_minute: EventHandler<u8>,
    on_update_grace: EventHandler<u16>,
    on_toggle_weekday: EventHandler<u8>,
    on_update_level: EventHandler<AlarmLevel>,
    on_update_target: EventHandler<String>,
) -> Element {
    let blocked_id_for_request = cooldown_blocked_id.clone();
    let blocked_id_for_force = cooldown_blocked_id.clone();

    rsx! {
        div {
            if permission_blocked_n3 {
                div {
                    style: "background: #FEF3F2; border: 1px solid #FECDCA; padding: 12px; border-radius: 10px; margin-bottom: 14px; display: flex; justify-content: space-between; align-items: center; gap: 10px;",
                    span {
                        style: "font-size: 12px; color: #B42318;",
                        "Conceda as 6 permissões antes de ativar um hábito Nível 3."
                    }
                    button {
                        class: "btn-secondary",
                        onclick: move |_| on_goto_permissions.call(()),
                        "Permissões"
                    }
                }
            }

            if cooldown_blocked_id.is_some() {
                div {
                    style: "background: #FFFAEB; border: 1px solid #FEDF89; padding: 14px; border-radius: 10px; margin-bottom: 14px; display: flex; flex-direction: column; gap: 10px;",
                    div {
                        style: "font-size: 13px; font-weight: 600; color: #B54708;",
                        "🛡️ Hábito Nível 3 protegido por bloqueio"
                    }
                    div {
                        style: "font-size: 12px; color: #475467; line-height: 1.4;",
                        "Para evitar desistir de acordar com preguiça, o Nível 3 trava edições impulsivas. Pode pedir o desbloqueio normal ou forçar a eliminação definitiva agora."
                    }
                    div {
                        style: "display: flex; gap: 8px; flex-wrap: wrap;",
                        button {
                            class: "btn-secondary",
                            onclick: move |_| {
                                if let Some(id) = &blocked_id_for_request {
                                    on_request_cooldown_unlock.call(id.clone());
                                }
                            },
                            "Pedir desbloqueio (24h)"
                        }
                        button {
                            class: "btn-danger-ghost",
                            onclick: move |_| {
                                if let Some(id) = &blocked_id_for_force {
                                    on_force_delete_alarm.call(id.clone());
                                }
                            },
                            "Forçar eliminação agora"
                        }
                    }
                }
            }

            // Habit Editor Card
            div {
                class: "surface-pad",
                h2 {
                    style: "margin: 0 0 12px 0; font-size: 15px; font-weight: 600; color: #101828;",
                    if editing_id.is_some() { "Editar hábito" } else { "Novo hábito" }
                }

                div {
                    style: "margin-bottom: 12px;",
                    label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 4px;", "Nome" }
                    input {
                        class: "field-input",
                        r#type: "text",
                        value: "{title}",
                        oninput: move |evt| on_update_title.call(evt.value())
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
                                    on_update_hour.call(v.min(23));
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
                                    on_update_minute.call(v.min(59));
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
                                    on_update_grace.call(v.clamp(1, 240));
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
                                style: if (weekdays_mask & bit) != 0 {
                                    "padding: 6px 10px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer;"
                                } else {
                                    "padding: 6px 10px; background: #FFFFFF; color: #475467; border: 1px solid #D0D5DD; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer;"
                                },
                                onclick: move |_| on_toggle_weekday.call(bit),
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
                                style: if level == lvl {
                                    "flex: 1; padding: 8px 6px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 7px; font-weight: 500; font-size: 12px; cursor: pointer;"
                                } else {
                                    "flex: 1; padding: 8px 6px; background: #FFFFFF; color: #475467; border: 1px solid #D0D5DD; border-radius: 7px; font-weight: 500; font-size: 12px; cursor: pointer;"
                                },
                                onclick: move |_| on_update_level.call(lvl),
                                "{label_txt}"
                            }
                        }
                    }
                }

                if level == AlarmLevel::ProofOfAction {
                    div {
                        style: "margin-bottom: 14px; background: #F9FAFB; border: 1px solid #EAECF0; padding: 10px; border-radius: 8px;",
                        label { style: "display: block; font-size: 12px; font-weight: 500; color: #344054; margin-bottom: 6px;", "Objeto para validação visual" }
                        div {
                            style: "display: flex; gap: 6px; flex-wrap: wrap;",
                            for (code, friendly) in CURATED_OBJECT_LABELS.iter().copied() {
                                button {
                                    style: if target_label == code {
                                        "padding: 5px 10px; background: #101828; color: #FFFFFF; border: 1px solid #101828; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer;"
                                    } else {
                                        "padding: 5px 10px; background: #FFFFFF; color: #344054; border: 1px solid #D0D5DD; border-radius: 6px; font-size: 12px; font-weight: 500; cursor: pointer;"
                                    },
                                    onclick: move |_| on_update_target.call(code.to_string()),
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
                        style: "flex: 1; cursor: pointer;",
                        onclick: move |_| on_save_alarm.call(()),
                        if editing_id.is_some() { "Guardar alterações" } else { "Adicionar hábito" }
                    }
                    if editing_id.is_some() {
                        button {
                            class: "btn-secondary",
                            style: "cursor: pointer;",
                            onclick: move |_| on_cancel_edit.call(()),
                            "Cancelar"
                        }
                    }
                }
            }

            // Habits List
            div {
                class: "section-head",
                h2 { class: "section-title", "Todos os hábitos" }
                span { class: "section-meta", "{alarms.len()}" }
            }
            div {
                class: "surface-group",
                if alarms.is_empty() {
                    div {
                        style: "padding: 20px; text-align: center; color: #667085; font-size: 13px;",
                        "Nenhum hábito configurado."
                    }
                }
                for item in alarms {
                    {
                        let id_for_toggle = item.id.clone();
                        let item_level = item.level;
                        let next_enabled = !item.enabled;
                        let item_for_edit = item.clone();
                        let id_for_delete = item.id.clone();
                        let id_for_test = item.id.clone();
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
                                        style: "cursor: pointer;",
                                        onclick: move |_| on_toggle_enabled.call((id_for_toggle.clone(), next_enabled, item_level)),
                                        if item.enabled { "Pausar" } else { "Ativar" }
                                    }

                                    button {
                                        class: "btn-secondary",
                                        style: "cursor: pointer;",
                                        onclick: move |_| on_edit_alarm.call(item_for_edit.clone()),
                                        "Editar"
                                    }

                                    button {
                                        class: "btn-secondary",
                                        style: "cursor: pointer;",
                                        onclick: move |_| on_test_alarm.call(id_for_test.clone()),
                                        "Testar +10s"
                                    }

                                    button {
                                        class: "btn-danger-ghost",
                                        style: "cursor: pointer;",
                                        onclick: move |_| on_delete_alarm.call(id_for_delete.clone()),
                                        "Eliminar"
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
