use std::collections::HashMap;

use crate::view::format_duration;

pub struct Log {
    pub start: u64,
    pub end: Option<u64>,
    pub class: String,
    pub title: String,
}

impl Log {
    pub fn new(start: u64, end: Option<u64>, class: String, title: String) -> Self {
        Self {
            start,
            end,
            class,
            title,
        }
    }

    pub fn duration(&self) -> u64 {
        self.end
            .unwrap_or(chrono::Utc::now().timestamp_millis() as u64)
            - self.start
    }
}

pub struct Title {
    pub title: String,
    pub logs: Vec<usize>,
    total_duration: u64,
}

impl Title {
    pub fn new(title: String) -> Self {
        Self {
            title,
            logs: Vec::new(),
            total_duration: 0,
        }
    }

    pub fn add_log(&mut self, log_index: usize, log_duration: u64) {
        self.logs.push(log_index);
        self.total_duration = self.total_duration + log_duration;
    }

    pub fn update_log_duration(&mut self, duration: u64) {
        self.total_duration += duration;
    }

    pub fn total_duration(&self, logs: &Vec<Log>) -> u64 {
        let mut duration = self.total_duration;
        if let Some(last_index) = self.logs.last() {
            let last = &logs[*last_index];
            if last.end.is_none() {
                duration += last.duration();
            }
        }
        duration
    }
}

pub struct Class {
    pub class: String,
    pub titles: Vec<Title>,
    title_map: HashMap<String, usize>,
}

impl Class {
    pub fn new(class: String) -> Self {
        Self {
            class,
            titles: Vec::new(),
            title_map: HashMap::new(),
        }
    }

    pub fn get_title_mut(&mut self, title: String) -> &mut Title {
        if let Some(title_index) = self.title_map.get(&title) {
            &mut self.titles[*title_index]
        } else {
            let title_index = self.titles.len();
            self.title_map.insert(title.clone(), title_index);
            self.titles.push(Title::new(title));
            &mut self.titles[title_index]
        }
    }

    pub fn total_duration(&self, logs: &Vec<Log>) -> u64 {
        self.titles.iter().map(|t| t.total_duration(logs)).sum()
    }

    pub fn sort(&mut self, logs: &Vec<Log>) {
        self.titles
            .sort_by(|a, b| b.total_duration(logs).cmp(&a.total_duration(logs)));
    }

    pub fn index_of(&self, title: &str) -> Option<usize> {
        self.titles.iter().position(|t| t.title == title)
    }

    pub fn iter_logs<'a>(&'a self, logs: &'a Vec<Log>) -> impl Iterator<Item = &Log> + 'a {
        self.titles
            .iter()
            .flat_map(move |title| title.logs.iter().map(move |&log_index| &logs[log_index]))
    }
}

pub struct Model {
    pub classes: Vec<Class>,
    class_map: HashMap<String, usize>,
    pub logs: Vec<Log>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            class_map: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn get_class_mut(&mut self, class: String) -> &mut Class {
        if let Some(class_index) = self.class_map.get(&class) {
            &mut self.classes[*class_index]
        } else {
            let class_index = self.classes.len();
            self.class_map.insert(class.clone(), class_index);
            self.classes.push(Class::new(class));
            &mut self.classes[class_index]
        }
    }

    pub fn add_log(&mut self, log: Log) {
        let class_string = log.class.clone();
        let title_string = log.title.clone();
        let mut log_duration = log.duration();
        if log.end.is_none() {
            log_duration = 0;
        }
        self.logs.push(log);

        let log_index = self.logs.len() - 1;

        self.get_class_mut(class_string)
            .get_title_mut(title_string)
            .add_log(log_index, log_duration);
    }

    pub fn complete_log(&mut self, replacement_log: Log) {
        let class_string = replacement_log.class.clone();
        let title_string = replacement_log.title.clone();

        // calculate new duration
        let newest_log = self.logs.last_mut().unwrap();
        let duration = replacement_log.duration();
        // Replace old log
        *newest_log = replacement_log;
        // Update total duration
        self.get_class_mut(class_string)
            .get_title_mut(title_string)
            .update_log_duration(duration);
    }

    pub fn total_duration(&self) -> u64 {
        self.classes
            .iter()
            .map(|c| c.total_duration(&self.logs))
            .sum()
    }

    pub fn sort(&mut self) {
        self.classes.sort_by(|a, b| {
            b.total_duration(&self.logs)
                .cmp(&a.total_duration(&self.logs))
        });
        for class in self.classes.iter_mut() {
            class.sort(&self.logs);
        }
    }

    pub fn print(&self) {
        for class in self.classes.iter() {
            println!(
                "{}: {}",
                class.class,
                format_duration(class.total_duration(&self.logs))
            );
            for title in class.titles.iter() {
                println!(
                    "  {}: {}",
                    title.title,
                    format_duration(title.total_duration)
                );
            }
        }
        println!("Total: {}", format_duration(self.total_duration()));
    }

    pub fn index_of(&self, class: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.class == class)
    }

    pub fn iter_all_logs(&self) -> impl Iterator<Item = &Log> {
        self.classes.iter().flat_map(|class| {
            class
                .titles
                .iter()
                .flat_map(|title| title.logs.iter().map(|&log_index| &self.logs[log_index]))
        })
    }
}
