mod config;
mod daemon_commands;
mod interval;
mod log_parsing;
mod log_reader;
mod tui;
mod view;

use daemon_commands::send_command;
use std::env;
use view::render_log;

use crate::{config::Config, interval::Interval, tui::start_tui};

fn main() {
    // use chrono::Utc;
    // let start = Utc::now().timestamp_millis();
    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--idle") => send_command("idle"),
        Some("--resume") => send_command("resume"),
        Some("--help") | Some("-h") => {
            print_usage();
        }
        None => {
            // view::render_log(&Settings::new());
            start_tui(Settings::new()).unwrap();
        }
        _ => {
            let mut settings = Settings::new();
            // Are we waiting on values for these args?
            let mut class = false;
            let mut days = false;
            for arg in args.iter().skip(1) {
                if class {
                    settings.class_arg = match settings.config.class_mappings.get(arg) {
                        Some(filtered_class) => filtered_class.clone(),
                        None => arg.clone(),
                    };
                    class = false;
                } else if days {
                    match arg.clone().parse::<u64>() {
                        Ok(day_count) => {
                            settings.interval.set_days(day_count);
                            days = false;
                        }
                        Err(_) => {
                            println!("Invalid value for the days argument.");
                            return;
                        }
                    }
                } else {
                    match arg.as_str() {
                        "--class" | "-c" => {
                            class = true;
                        }
                        "--days" | "-d" => {
                            days = true;
                        }
                        "--full" | "-f" => {
                            settings.full = true;
                        }
                        "--multi" | "-m" => {
                            settings.multi_timeline = true;
                        }

                        arg => {
                            eprintln!("Unknown argument: {arg}");
                            print_usage();
                            std::process::exit(1);
                        }
                    }
                }
            }

            if class {
                println!("Please provied a class name for the class argument.");
                return;
            }
            if days {
                println!("Please provied a day count for the days argument.");
                return;
            }

            // render_log(&settings);
            start_tui(settings).unwrap();
        }
    }

    // let end = Utc::now().timestamp_millis();
    // println!("Runtime: {}ms", end - start)
}

fn print_usage() {
    println!(
        "Usage: hyprlog\n
        [ --help | -h ]\n
        [ --full | -f ]\n
        [ --multi | -m ]\n
        [ --days DAY_COUNT | -d DAY_COUNT ]\n
        [ --class CLASS_NAME | -c CLASS_NAME ]\n
        [ --idle | --resume]"
    );
}

pub struct Settings {
    pub full: bool,
    pub multi_timeline: bool,
    pub class_arg: String,
    pub interval: Interval,
    pub config: Config,
}

impl Settings {
    fn new() -> Self {
        Self {
            full: false,
            multi_timeline: false,
            class_arg: String::from(""),
            interval: Interval::default(),
            config: Config::new(),
        }
    }
}
