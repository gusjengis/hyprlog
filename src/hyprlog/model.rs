use std::collections::HashMap;

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

    fn add_duration(&mut self, duration: u64) {
        self.total_duration += duration;
    }

    fn class<'a>(&self, logs: &'a Vec<Log>) -> &'a String {
        &logs[*self.logs.first().unwrap()].class
    }
}

pub struct Class {
    pub class: String,
    pub titles: Vec<Title>,
    title_map: HashMap<String, usize>,
    pub logs: Vec<usize>,
}

impl Class {
    pub fn new(class: String) -> Self {
        Self {
            class,
            titles: Vec::new(),
            title_map: HashMap::new(),
            logs: Vec::new(),
        }
    }

    pub fn get_title(&self, class: &String) -> Option<&Title> {
        if let Some(title_index) = self.title_map.get(class) {
            Some(&self.titles[*title_index])
        } else {
            None
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
        self.title_map.clear();
        for (i, title) in self.titles.iter().enumerate() {
            self.title_map.insert(title.title.clone(), i);
        }
        self.logs
            .sort_by(|a, b| logs[*a].start.cmp(&logs[*b].start));
    }

    pub fn index_of(&self, title: &str) -> Option<usize> {
        self.titles.iter().position(|t| t.title == title)
    }

    pub fn iter_logs<'a>(&'a self, logs: &'a Vec<Log>) -> impl Iterator<Item = &Log> + 'a {
        self.logs.iter().map(move |&log_index| &logs[log_index])
    }

    fn add_log(&mut self, log_index: usize, title_string: String, log_duration: u64) {
        self.logs.push(log_index);
        self.get_title_mut(title_string)
            .add_log(log_index, log_duration);
    }
}

pub struct Model {
    pub classes: Vec<Class>,
    class_map: HashMap<String, usize>,
    pub logs: Vec<Log>,
    pub titles: Vec<(usize, usize)>,
}

impl Model {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            class_map: HashMap::new(),
            logs: Vec::new(),
            titles: Vec::new(),
        }
    }

    pub fn get_class(&self, class: &String) -> Option<&Class> {
        if let Some(class_index) = self.class_map.get(class) {
            Some(&self.classes[*class_index])
        } else {
            None
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

    pub fn add_log(&mut self, log: Log, bulk_addition: bool) {
        let mut changed_log_indices = vec![];
        if let Some(last_log) = self.logs.last_mut() {
            if last_log.end.is_none() {
                last_log.end = Some(log.start);
            }
            let class_string = last_log.class.clone();
            let title_string = last_log.title.clone();
            let duration = last_log.duration();
            self.get_class_mut(class_string)
                .get_title_mut(title_string)
                .add_duration(duration);
            changed_log_indices.push(self.logs.len() - 1);
        }
        let class_string = log.class.clone();
        let title_string = log.title.clone();
        let mut log_duration = log.duration();
        if log.end.is_none() {
            log_duration = 0;
        }
        self.logs.push(log);

        let log_index = self.logs.len() - 1;

        self.get_class_mut(class_string)
            .add_log(log_index, title_string, log_duration);
        changed_log_indices.push(log_index);
        if !bulk_addition {
            self.maintain_order(changed_log_indices);
        }
    }

    pub fn sort(&mut self) {
        self.classes.sort_by(|a, b| {
            b.total_duration(&self.logs)
                .cmp(&a.total_duration(&self.logs))
        });
        self.class_map.clear();
        for (i, class) in self.classes.iter().enumerate() {
            self.class_map.insert(class.class.clone(), i);
        }
        for class in self.classes.iter_mut() {
            class.sort(&self.logs);
        }

        self.build_titles();
        self.titles.sort_by(|b, a| {
            self.classes[a.0].titles[a.1]
                .total_duration(&self.logs)
                .cmp(&self.classes[b.0].titles[b.1].total_duration(&self.logs))
        });
    }

    fn build_titles(&mut self) {
        for (i, class) in self.classes.iter().enumerate() {
            for (j, _) in class.titles.iter().enumerate() {
                self.titles.push((i, j));
            }
        }
    }

    // pub fn print(&self) {
    //     for class in self.classes.iter() {
    //         println!(
    //             "{}: {}",
    //             class.class,
    //             format_duration(class.total_duration(&self.logs))
    //         );
    //         for title in class.titles.iter() {
    //             println!(
    //                 "  {}: {}",
    //                 title.title,
    //                 format_duration(title.total_duration)
    //             );
    //         }
    //     }
    //     println!("Total: {}", format_duration(self.total_duration()));
    // }

    pub fn index_of(&self, class: &str) -> Option<usize> {
        self.classes.iter().position(|c| c.class == class)
    }

    pub fn logs_iter(&self) -> impl Iterator<Item = &Log> {
        self.logs.iter()
    }
    pub fn get_title(&self, class_name: &String, title_name: &String) -> Option<&Title> {
        if let Some(class) = self.get_class(class_name) {
            class.get_title(title_name)
        } else {
            None
        }
    }

    pub fn maintain_order(&mut self, changed_log_indices: Vec<usize>) {
        use std::collections::HashSet;

        let mut changed_class_indices: HashSet<usize> = HashSet::new();
        for &log_idx in &changed_log_indices {
            if log_idx < self.logs.len() {
                let log = &self.logs[log_idx];
                if let Some(&class_idx) = self.class_map.get(&log.class) {
                    changed_class_indices.insert(class_idx);
                }
            }
        }

        for class_idx in changed_class_indices.clone() {
            if class_idx >= self.classes.len() {
                continue;
            }

            let class = &mut self.classes[class_idx];
            let mut i = 0;
            while i < class.titles.len() {
                let title_duration = class.titles[i].total_duration(&self.logs);
                if i > 0 {
                    let prev_duration = class.titles[i - 1].total_duration(&self.logs);
                    if title_duration > prev_duration {
                        class.titles.swap(i, i - 1);
                        let prev_title_name = class.titles[i].title.clone();
                        class.title_map.insert(prev_title_name, i);
                        let curr_title_name = class.titles[i - 1].title.clone();
                        class.title_map.insert(curr_title_name, i - 1);
                        continue;
                    }
                }
                i += 1;
            }
        }

        let mut changed_class_indices: Vec<_> = changed_class_indices.into_iter().collect();
        changed_class_indices.sort();
        let mut i = 0;
        while i < changed_class_indices.len() {
            let class_idx = changed_class_indices[i];
            if class_idx >= self.classes.len() {
                i += 1;
                continue;
            }

            let class = &self.classes[class_idx];
            let class_duration = class.total_duration(&self.logs);

            if class_idx > 0 {
                let prev_class = &self.classes[class_idx - 1];
                let prev_duration = prev_class.total_duration(&self.logs);
                if class_duration > prev_duration {
                    self.classes.swap(class_idx, class_idx - 1);

                    let class_name = self.classes[class_idx].class.clone();
                    self.class_map.insert(class_name, class_idx);
                    let prev_class_name = self.classes[class_idx - 1].class.clone();
                    self.class_map.insert(prev_class_name, class_idx - 1);

                    if !changed_class_indices.contains(&(class_idx - 1)) {
                        changed_class_indices.push(class_idx - 1);
                        changed_class_indices.sort();
                    }
                    continue;
                }
            }
            i += 1;
        }
    }
}
