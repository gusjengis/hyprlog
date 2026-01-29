use std::error::Error;

use crate::{
    log_reader::LogReader,
    model::{Log, Model},
    Settings,
};

pub fn build_model(
    model: &mut Model,
    log_reader: &mut LogReader,
    settings: &Settings,
) -> Result<(), Box<dyn Error>> {
    let mut last_timestamp = None;
    let mut last_class: Option<String> = None;
    let mut last_title: Option<String> = None;

    let _ = log_reader.reset();
    for row in log_reader {
        let record = row.unwrap();
        let timestamp: u64 = (record[0].parse::<i64>()?) as u64;
        let mut class = record[1].to_string();
        let title = record[2].to_string();

        // filter classes using hashmap to rename them according to config
        class = match settings.config.class_mappings.get(&class) {
            Some(filtered_class) => filtered_class.clone(),
            None => class,
        };

        if class.contains("steam_app") {
            class = String::from("steam");
        }

        if class == "SYSTEM" {
            match title.as_str() {
                "boot" => {
                    last_timestamp = None;
                    last_class = None;
                    last_title = None;
                }
                "resume" => {
                    last_timestamp = Some(timestamp);
                }
                "shutdown" | "idle" => {
                    if let (Some(start), Some(class), Some(title)) =
                        (last_timestamp, last_class.as_ref(), last_title.as_ref())
                    {
                        model.add_log(Log::new(
                            start,
                            Some(timestamp),
                            class.clone(),
                            title.clone(),
                        ));
                    }
                    last_timestamp = None;
                }
                _ => {}
            }
        } else {
            if let (Some(start), Some(class), Some(title)) =
                (last_timestamp, last_class.as_ref(), last_title.as_ref())
            {
                model.add_log(Log::new(
                    start,
                    Some(timestamp),
                    class.clone(),
                    title.clone(),
                ));
            }

            last_timestamp = Some(timestamp);
            last_class = Some(class.clone());
            last_title = Some(title.clone());
        }
    }
    if let (Some(start), Some(class), Some(title)) =
        (last_timestamp, last_class.as_ref(), last_title.as_ref())
    {
        model.add_log(Log::new(start, None, class.clone(), title.clone()));
    }
    model.sort();

    Ok(())
}

pub fn update_model(
    model: &mut Model,
    log_reader: &mut LogReader,
    settings: &Settings,
) -> Result<(), Box<dyn Error>> {
    update_model_append(model, log_reader, settings)?;
    Ok(())
}

pub fn update_model_append(
    model: &mut Model,
    log_reader: &mut LogReader,
    settings: &Settings,
) -> Result<(), Box<dyn Error>> {
    let mut last_timestamp = None;
    let mut last_class: Option<String> = None;
    let mut last_title: Option<String> = None;

    let newest_log = model
        .logs
        .last()
        .expect("update_model_append(): no logs in model");

    for row in &mut *log_reader {
        let record = row.unwrap();
        let timestamp: u64 = (record[0].parse::<i64>()?) as u64;
        let mut class = record[1].to_string();
        let title = record[2].to_string();
        class = filter_class(class, settings);
        if timestamp == newest_log.start && class == newest_log.class && title == newest_log.title {
            // we've walked back to where we left off when last building or updating the model
            if let Some(newer_log) = log_reader.next() {
                let record = newer_log.unwrap();
                let timestamp: u64 = (record[0].parse::<i64>()?) as u64;
                let mut class = record[1].to_string();
                let title = record[2].to_string();
                class = filter_class(class, settings);
                last_timestamp = Some(timestamp);
                last_class = Some(class);
                last_title = Some(title);

                let updated_log = Log {
                    start: newest_log.start,
                    end: Some(timestamp),
                    class: newest_log.class.clone(),
                    title: newest_log.title.clone(),
                };
                model.complete_log(updated_log);
                break;
            }
            return Ok(());
        }
    }
    if last_timestamp.is_none() {
        return Ok(());
    }
    for row in log_reader {
        let record = row.unwrap();
        let timestamp: u64 = (record[0].parse::<i64>()?) as u64;
        let mut class = record[1].to_string();
        let title = record[2].to_string();

        class = filter_class(class, settings);

        if class == "SYSTEM" {
            match title.as_str() {
                "boot" => {
                    last_timestamp = None;
                    last_class = None;
                    last_title = None;
                }
                "resume" => {
                    last_timestamp = Some(timestamp);
                }
                "shutdown" | "idle" => {
                    if let (Some(start), Some(class), Some(title)) =
                        (last_timestamp, last_class.as_ref(), last_title.as_ref())
                    {
                        model.add_log(Log::new(
                            start,
                            Some(timestamp),
                            class.clone(),
                            title.clone(),
                        ));
                    }
                    last_timestamp = None;
                }
                _ => {}
            }
        } else {
            if let (Some(start), Some(class), Some(title)) =
                (last_timestamp, last_class.as_ref(), last_title.as_ref())
            {
                model.add_log(Log::new(
                    start,
                    Some(timestamp),
                    class.clone(),
                    title.clone(),
                ));
            }

            last_timestamp = Some(timestamp);
            last_class = Some(class.clone());
            last_title = Some(title.clone());
        }
    }
    if let (Some(start), Some(class), Some(title)) =
        (last_timestamp, last_class.as_ref(), last_title.as_ref())
    {
        model.add_log(Log::new(start, None, class.clone(), title.clone()));
    }
    model.sort();

    Ok(())
}

fn filter_class(class: String, settings: &Settings) -> String {
    // filter classes using hashmap to rename them according to config
    let mut res = match settings.config.class_mappings.get(&class) {
        Some(filtered_class) => filtered_class.clone(),
        None => class.clone(),
    };

    if class.contains("steam_app") {
        res = String::from("steam");
    }

    return res;
}
