use crabcheck_core::{
    format_local_time_hm, format_local_time_hms, AlarmLevel, DailyTimelineSnapshot, StreakSummary,
};
use dioxus::prelude::*;

#[component]
pub fn StreakCard(streak: StreakSummary) -> Element {
    rsx! {
        div {
            class: "streak-banner",
            div {
                class: "streak-stat-col",
                div { class: "streak-stat-val", "{streak.current_streak_days}d" }
                div { class: "streak-stat-label", "Sequência Atual" }
            }
            div { style: "width: 1px; height: 28px; background: #E4E7EC;" }
            div {
                class: "streak-stat-col",
                div { class: "streak-stat-val", "{streak.best_streak_days}d" }
                div { class: "streak-stat-label", "Melhor Sequência" }
            }
            div { style: "width: 1px; height: 28px; background: #E4E7EC;" }
            div {
                class: "streak-stat-col",
                div { class: "streak-stat-val", "{streak.adherence_rate}%" }
                div { class: "streak-stat-label", "Adesão Geral" }
            }
        }
    }
}

#[component]
pub fn TimelineView(
    snap: DailyTimelineSnapshot,
    streak: StreakSummary,
    note_draft: String,
    active_tag: Option<String>,
    on_input_note: EventHandler<String>,
    on_save_note: EventHandler<()>,
    on_clear_tag: EventHandler<()>,
    on_select_tag: EventHandler<String>,
    on_test_alarm: EventHandler<String>,
    on_delete_note: EventHandler<String>,
) -> Element {
    rsx! {
        div {
            StreakCard { streak }

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
                        oninput: move |evt| on_input_note.call(evt.value())
                    }
                    button {
                        class: "btn-primary",
                        style: "white-space: nowrap;",
                        onclick: move |_| on_save_note.call(()),
                        "Guardar"
                    }
                }
            }

            // Active #tag filter
            if let Some(tag) = active_tag {
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
                        onclick: move |_| on_clear_tag.call(()),
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
                                    onclick: move |_| on_test_alarm.call(id_for_test.clone()),
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
                                        style: "font-size: 12px; color: #475467; margin-top: 2px;",
                                        if let Some(conf) = out.confidence {
                                            "Objeto validado: {lbl} ({conf * 100.0:.0}%)"
                                        } else {
                                            "Objeto validado: {lbl}"
                                        }
                                    }
                                }
                                if out.proof_image_path.is_some() {
                                    div {
                                        style: "font-size: 11px; color: #027A48; margin-top: 3px;",
                                        "📸 Prova visual registada com sucesso"
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
                                        style: "background: transparent; border: none; color: #667085; font-size: 12px; padding: 0; cursor: pointer;",
                                        onclick: move |_| on_delete_note.call(note_id.clone()),
                                        "Remover"
                                    }
                                }
                                div {
                                    style: "font-size: 14px; color: #101828; line-height: 1.4;",
                                    "{note.body}"
                                }
                                if !note.tags.is_empty() {
                                    div {
                                        style: "display: flex; flex-wrap: wrap; gap: 6px; margin-top: 2px;",
                                        for tag in note.tags.iter() {
                                            {
                                                let t_click = tag.clone();
                                                rsx! {
                                                    button {
                                                        key: "tag-{tag}",
                                                        style: "padding: 2px 7px; background: #F2F4F7; color: #344054; border: 1px solid #E4E7EC; border-radius: 6px; font-size: 11px; cursor: pointer;",
                                                        onclick: move |_| on_select_tag.call(t_click.clone()),
                                                        "#{tag}"
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
}
