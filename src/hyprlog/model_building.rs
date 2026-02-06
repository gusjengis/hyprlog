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
        if settings
            .loaded_interval
            .contains_utc_timestamp_millis(timestamp)
        {
            continue;
        }
        let mut class = record[1].to_string();
        let title = record[2].to_string();

        // filter classes using hashmap to rename them according to config
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
                        model.add_log(
                            Log::new(start, Some(timestamp), class.clone(), title.clone()),
                            true,
                        );
                    }
                    last_timestamp = None;
                }
                _ => {}
            }
        } else {
            if let (Some(start), Some(class), Some(title)) =
                (last_timestamp, last_class.as_ref(), last_title.as_ref())
            {
                model.add_log(
                    Log::new(start, Some(timestamp), class.clone(), title.clone()),
                    true,
                );
            }

            last_timestamp = Some(timestamp);
            last_class = Some(class.clone());
            last_title = Some(title.clone());
        }
    }
    if let (Some(start), Some(class), Some(title)) =
        (last_timestamp, last_class.as_ref(), last_title.as_ref())
    {
        model.add_log(Log::new(start, None, class.clone(), title.clone()), true);
    }
    model.sort();

    Ok(())
}

pub fn filter_class(class: String, settings: &Settings) -> String {
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
