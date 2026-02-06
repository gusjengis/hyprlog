use crate::model::Log;
use crate::{model::Model, Settings};

fn get_relevant_logs<'a>(
    model: &'a Model,
    settings: &'a Settings,
    title: Option<&'a String>,
) -> Vec<&'a Log> {
    if let Some(title_str) = title {
        if settings.class_arg.is_empty() {
            if let Some(class) = model.get_class(title_str) {
                class.iter_logs(&model.logs).collect()
            } else {
                Vec::new()
            }
        } else if let Some(title_obj) = model.get_title(&settings.class_arg, title_str) {
            title_obj
                .logs
                .iter()
                .map(|&log_index| &model.logs[log_index])
                .collect()
        } else {
            Vec::new()
        }
    } else if settings.class_arg.is_empty() {
        model.logs_iter(&settings.focused_interval).collect()
    } else if let Some(class) = model.get_class(&settings.class_arg) {
        class.iter_logs(&model.logs).collect()
    } else {
        Vec::new()
    }
}

pub fn timeline(
    model: &Model,
    width: usize,
    settings: &Settings,
    title: Option<&String>,
) -> Vec<(String, i64, i64, bool, bool)> {
    let ms_per_section = (settings.focused_interval.width() / width as u64) as u64;
    let starting_ms = settings.focused_interval.start.timestamp_millis() as u64;
    let mut sections: Vec<(String, i64, i64, bool, bool)> =
        vec![(String::from(""), 0, 0, false, false); width];

    let logs = get_relevant_logs(model, settings, title);

    for log in logs {
        if let Some(end) = log.end {
            assign_interval_to_section(
                log.start,
                end,
                &log.class,
                &log.title,
                starting_ms,
                ms_per_section,
                settings,
                title,
                &mut sections,
            );
        } else {
            break;
        }
    }

    if let Some(last_log) = model.logs.last() {
        if last_log.end.is_none() {
            let timestamp = chrono::Utc::now().timestamp_millis() as u64;
            assign_interval_to_section(
                last_log.start,
                timestamp,
                &last_log.class,
                &last_log.title,
                starting_ms,
                ms_per_section,
                settings,
                title,
                &mut sections,
            );
        }
    }

    sections
}

fn assign_interval_to_section(
    start: u64,
    end: u64,
    class_name: &String,
    title: &String,
    starting_ms: u64,
    ms_per_section: u64,
    settings: &Settings,
    label: Option<&String>,
    sections: &mut Vec<(String, i64, i64, bool, bool)>,
) {
    if settings.multi_timeline && (label.unwrap() == &key(settings, class_name, title))
        || !settings.multi_timeline
            && (settings.class_arg == "" || settings.full || &settings.class_arg == class_name)
    {
        let edge_detection_padding = (ms_per_section as f64 / 10.0) as u64;
        let start_index = section_index(starting_ms, ms_per_section, start);
        let end_index = section_index(starting_ms, ms_per_section, end);
        for i in start_index..end_index + 1 {
            let section_start = starting_ms + ms_per_section * i as u64;
            let section_end = starting_ms + ms_per_section * (i as u64 + 1);
            let contribution = (section_end.min(end) - section_start.max(start)) as i64;
            if section_start + edge_detection_padding >= start {
                sections[i].3 = true;
            }
            if section_end - edge_detection_padding <= end {
                sections[i].4 = true;
            }
            let key = key(settings, class_name, title);
            sections[i].2 += contribution;
            if sections[i].0 == key {
                sections[i].1 += contribution;
            } else {
                sections[i].1 -= contribution;
                if sections[i].1 < 0 {
                    sections[i].0 = key.clone();
                    sections[i].1 *= -1;
                }
            }
        }
    }
}

fn section_index(starting_ms: u64, ms_per_section: u64, timestamp: u64) -> usize {
    ((timestamp - starting_ms) / ms_per_section) as usize
}

fn key(settings: &Settings, last_class: &String, last_title: &String) -> String {
    if settings.full {
        format!("{last_class}: {last_title}")
    } else if settings.class_arg == "" {
        last_class.clone()
    } else {
        last_title.clone()
    }
}
