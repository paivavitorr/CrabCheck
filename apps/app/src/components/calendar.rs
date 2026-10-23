use chrono::{Datelike, Days, Local, LocalResult, NaiveDate, TimeZone};
use dioxus::prelude::*;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalendarWeekDay {
    pub offset: i64,
    pub date_iso: String,
    pub day_num: u32,
    pub weekday_short: &'static str,
    pub is_today: bool,
    pub is_selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonthGridDay {
    pub offset: i64,
    pub date_iso: String,
    pub day_num: u32,
    pub is_today: bool,
    pub is_selected: bool,
}

pub fn calculate_week_strip(now_epoch_ms: i64, selected_offset: i64) -> (String, Vec<CalendarWeekDay>) {
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
    let monday = selected_date
        .checked_sub_days(Days::new(weekday_idx as u64))
        .unwrap_or(selected_date);

    let weekday_labels = ["Seg", "Ter", "Qua", "Qui", "Sex", "Sáb", "Dom"];
    let mut days = Vec::with_capacity(7);

    for (i, label) in weekday_labels.iter().enumerate() {
        if let Some(d) = monday.checked_add_days(Days::new(i as u64)) {
            let offset = (d - today).num_days();
            days.push(CalendarWeekDay {
                offset,
                date_iso: d.format("%Y-%m-%d").to_string(),
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

pub fn calculate_month_grid(now_epoch_ms: i64, selected_offset: i64) -> (String, Vec<Option<MonthGridDay>>) {
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
    }
    .num_days() as u32;

    let mut grid = Vec::new();
    for _ in 0..start_col {
        grid.push(None);
    }
    for d in 1..=days_in_month {
        if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
            let offset = (date - today).num_days();
            grid.push(Some(MonthGridDay {
                offset,
                date_iso: date.format("%Y-%m-%d").to_string(),
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

pub fn prev_month_offset(now_ms: i64, current_offset: i64) -> i64 {
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

pub fn next_month_offset(now_ms: i64, current_offset: i64) -> i64 {
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

#[component]
pub fn CalendarCard(
    now_epoch_ms: i64,
    selected_offset: i64,
    show_month: bool,
    outcomes_map: HashMap<String, String>,
    on_toggle_month: EventHandler<()>,
    on_select_day: EventHandler<i64>,
) -> Element {
    let (week_title, week_days) = calculate_week_strip(now_epoch_ms, selected_offset);
    let (month_title, month_cells) = calculate_month_grid(now_epoch_ms, selected_offset);

    rsx! {
        div {
            class: "cal-card",
            div {
                class: "cal-top-row",
                button {
                    class: "cal-title-btn",
                    onclick: move |_| on_toggle_month.call(()),
                    span { if show_month { "{month_title}" } else { "{week_title}" } }
                    span { style: "font-size: 10px; color: #667085;", if show_month { "▴" } else { "▾" } }
                }
                div {
                    class: "cal-nav-group",
                    if selected_offset != 0 {
                        button {
                            class: "cal-btn-today",
                            onclick: move |_| on_select_day.call(0),
                            "Hoje"
                        }
                    }
                    button {
                        class: "cal-nav-arrow",
                        title: "Anterior",
                        onclick: move |_| {
                            let next = if show_month {
                                prev_month_offset(now_epoch_ms, selected_offset)
                            } else {
                                selected_offset - 7
                            };
                            on_select_day.call(next);
                        },
                        "◀"
                    }
                    button {
                        class: "cal-nav-arrow",
                        title: "Seguinte",
                        onclick: move |_| {
                            let next = if show_month {
                                next_month_offset(now_epoch_ms, selected_offset)
                            } else {
                                selected_offset + 7
                            };
                            on_select_day.call(next);
                        },
                        "▶"
                    }
                }
            }

            if !show_month {
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
                            let maybe_dot = outcomes_map.get(&day.date_iso);
                            rsx! {
                                button {
                                    key: "{day.offset}",
                                    class: "{classes}",
                                    onclick: move |_| on_select_day.call(day_offset),
                                    span { class: "cal-day-weekday", "{day.weekday_short}" }
                                    span { class: "cal-day-num", "{day.day_num}" }
                                    if let Some(status) = maybe_dot {
                                        span {
                                            class: match status.as_str() {
                                                "CUMPRIDO" => "cal-dot cal-dot-ok",
                                                "FALHOU" => "cal-dot cal-dot-fail",
                                                _ => "cal-dot cal-dot-snooze",
                                            }
                                        }
                                    }
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
                                let maybe_dot = outcomes_map.get(&day.date_iso);
                                rsx! {
                                    button {
                                        key: "{idx}",
                                        class: "{classes}",
                                        onclick: move |_| on_select_day.call(day_offset),
                                        span { "{day.day_num}" }
                                        if let Some(status) = maybe_dot {
                                            span {
                                                class: match status.as_str() {
                                                    "CUMPRIDO" => "cal-dot cal-dot-ok",
                                                    "FALHOU" => "cal-dot cal-dot-fail",
                                                    _ => "cal-dot cal-dot-snooze",
                                                }
                                            }
                                        }
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
    }
}
