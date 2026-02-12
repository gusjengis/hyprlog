use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::interval::Interval;

pub struct Log {
    pub id: u64,
    pub start: u64,
    pub end: Option<u64>,
    pub class: String,
    pub title: String,
}

static NEXT_LOG_ID: AtomicU64 = AtomicU64::new(1);

impl Log {
    pub fn new(start: u64, end: Option<u64>, class: String, title: String) -> Self {
        Self {
            id: NEXT_LOG_ID.fetch_add(1, Ordering::Relaxed),
            start,
            end,
            class,
            title,
        }
    }

    pub fn duration(&self) -> u64 {
        (self
            .end
            .unwrap_or(chrono::Utc::now().timestamp_millis() as u64) as i64
            - self.start as i64)
            .max(0) as u64
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

    fn remove_duration(&mut self, duration: u64) {
        self.total_duration = self.total_duration.saturating_sub(duration);
    }
}

pub struct Class {
    pub class: String,
    pub titles: Vec<Title>,
    title_map: HashMap<String, usize>,
    pub logs: Vec<usize>,
    total_duration: u64,
}

impl Class {
    pub fn new(class: String) -> Self {
        Self {
            class,
            titles: Vec::new(),
            title_map: HashMap::new(),
            logs: Vec::new(),
            total_duration: 0,
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
        let mut duration = self.total_duration;
        if let Some(last_index) = self.logs.last() {
            let last = &logs[*last_index];
            if last.end.is_none() {
                duration += last.duration();
            }
        }
        duration
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

    pub fn iter_logs<'a>(&'a self, logs: &'a Vec<Log>) -> impl Iterator<Item = &'a Log> + 'a {
        self.logs.iter().map(move |&log_index| &logs[log_index])
    }

    fn add_log(&mut self, log_index: usize, title_string: String, log_duration: u64) {
        self.logs.push(log_index);
        self.total_duration += log_duration;
        self.get_title_mut(title_string)
            .add_log(log_index, log_duration);
    }

    fn add_duration(&mut self, duration: u64) {
        self.total_duration += duration;
    }

    fn remove_duration(&mut self, duration: u64) {
        self.total_duration = self.total_duration.saturating_sub(duration);
    }
}

pub struct Model {
    pub classes: Vec<Class>,
    class_map: HashMap<String, usize>,
    pub logs: Vec<Log>,
    logs_by_start: Vec<usize>,
    pub titles: Vec<(usize, usize)>,
    titles_dirty: bool,
    mapped_log_ids: HashSet<u64>,
    visible_start_idx: usize,
    visible_end_idx: usize,
    visible_bounds_initialized: bool,
}

impl Model {
    pub fn new() -> Self {
        Self {
            classes: Vec::new(),
            class_map: HashMap::new(),
            logs: Vec::new(),
            logs_by_start: Vec::new(),
            titles: Vec::new(),
            titles_dirty: true,
            mapped_log_ids: HashSet::new(),
            visible_start_idx: 0,
            visible_end_idx: 0,
            visible_bounds_initialized: false,
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

    pub fn add_log(&mut self, log: Log, bulk_addition: bool, focused_interval: &Interval) {
        self.titles_dirty = true;
        let mut changed_log_indices = vec![];
        let mut closed_previous: Option<(usize, String, String, u64)> = None;

        let last_index = self.logs.len().checked_sub(1);
        if let Some(last_log) = self.logs.last_mut() {
            if last_log.end.is_none() {
                last_log.end = Some(log.start);

                if self.mapped_log_ids.contains(&last_log.id) {
                    closed_previous = Some((
                        last_index.expect("last index should exist"),
                        last_log.class.clone(),
                        last_log.title.clone(),
                        last_log.duration(),
                    ));
                }
            }
        }

        if let Some((closed_idx, class_string, title_string, duration)) = closed_previous {
            let class = self.get_class_mut(class_string);
            class.add_duration(duration);
            class.get_title_mut(title_string).add_duration(duration);
            changed_log_indices.push(closed_idx);
        }

        self.logs.push(log);
        let log_index = self.logs.len() - 1;
        self.insert_log_in_start_order(log_index);

        if focused_interval.contains_log(&self.logs[log_index]) {
            self.map_log(log_index);
            changed_log_indices.push(log_index);
        }

        if !bulk_addition {
            if !changed_log_indices.is_empty() {
                self.maintain_order(changed_log_indices);
            }

            if self.visible_bounds_initialized {
                self.visible_end_idx = self.visible_end_idx.min(self.logs_by_start.len());
            }
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

        self.titles_dirty = false;
    }

    pub fn ensure_titles_sorted(&mut self) {
        if !self.titles_dirty {
            return;
        }
        self.build_titles();
        self.titles.sort_by(|b, a| {
            self.classes[a.0].titles[a.1]
                .total_duration(&self.logs)
                .cmp(&self.classes[b.0].titles[b.1].total_duration(&self.logs))
        });
        self.titles_dirty = false;
    }

    fn build_titles(&mut self) {
        self.titles.clear();
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

    pub fn logs_iter<'a>(&'a self, interval: &'a Interval) -> impl Iterator<Item = &'a Log> + 'a {
        self.logs.iter().filter(|log| interval.contains_log(log))
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

        // Ordering can change over time (e.g. a currently-open log duration increases), so treat
        // the global titles index as stale whenever we attempt to maintain ordering.
        self.titles_dirty = true;

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

    pub fn reset_mappings(&mut self) {
        self.classes.clear();
        self.class_map.clear();
        self.titles.clear();
        self.titles_dirty = true;
        self.mapped_log_ids.clear();
        self.visible_start_idx = 0;
        self.visible_end_idx = 0;
        self.visible_bounds_initialized = false;
    }

    pub fn map_log(&mut self, log_index: usize) {
        if log_index >= self.logs.len() {
            return;
        }

        self.titles_dirty = true;
        let log_id = self.logs[log_index].id;
        if !self.mapped_log_ids.insert(log_id) {
            return;
        }

        let log = &self.logs[log_index];
        let class_string = log.class.clone();
        let title_string = log.title.clone();
        let log_duration = if log.end.is_none() { 0 } else { log.duration() };

        self.get_class_mut(class_string)
            .add_log(log_index, title_string, log_duration);
    }

    fn unmap_log(&mut self, log_index: usize) {
        if log_index >= self.logs.len() {
            return;
        }

        let log = &self.logs[log_index];
        if !self.mapped_log_ids.remove(&log.id) {
            return;
        }

        self.titles_dirty = true;
        let class_string = log.class.clone();
        let title_string = log.title.clone();
        let log_duration = if log.end.is_none() { 0 } else { log.duration() };

        let Some(class_index) = self.class_map.get(&class_string).copied() else {
            return;
        };

        let remove_class = {
            let class = &mut self.classes[class_index];
            if let Some(pos) = class.logs.iter().position(|&idx| idx == log_index) {
                class.logs.swap_remove(pos);
            }
            class.remove_duration(log_duration);

            if let Some(title_index) = class.title_map.get(&title_string).copied() {
                {
                    let title = &mut class.titles[title_index];
                    if let Some(pos) = title.logs.iter().position(|&idx| idx == log_index) {
                        title.logs.swap_remove(pos);
                    }
                    title.remove_duration(log_duration);
                }

                if class.titles[title_index].logs.is_empty() {
                    class.titles.swap_remove(title_index);
                    class.title_map.remove(&title_string);
                    if title_index < class.titles.len() {
                        let swapped_title = class.titles[title_index].title.clone();
                        class.title_map.insert(swapped_title, title_index);
                    }
                }
            }

            class.logs.is_empty()
        };

        if remove_class {
            self.classes.swap_remove(class_index);
            self.class_map.remove(&class_string);
            if class_index < self.classes.len() {
                let swapped_class = self.classes[class_index].class.clone();
                self.class_map.insert(swapped_class, class_index);
            }
        }
    }

    fn lower_bound_start(&self, target_ms: u64) -> usize {
        let mut lo = 0usize;
        let mut hi = self.logs_by_start.len();

        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            let log_index = self.logs_by_start[mid];
            if self.logs[log_index].start < target_ms {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }

        lo
    }

    fn insert_log_in_start_order(&mut self, log_index: usize) {
        let log = &self.logs[log_index];
        let start = log.start;
        let id = log.id;

        let pos = self.logs_by_start.partition_point(|&idx| {
            let other = &self.logs[idx];
            if other.start == start {
                other.id < id
            } else {
                other.start < start
            }
        });

        self.logs_by_start.insert(pos, log_index);
    }

    fn visible_bounds(&self, interval: &Interval) -> (usize, usize) {
        let start_ms = interval.start.timestamp_millis().max(0) as u64;
        let end_ms = interval.end.timestamp_millis().max(0) as u64;
        (
            self.lower_bound_start(start_ms),
            self.lower_bound_start(end_ms),
        )
    }

    pub fn mark_visible_bounds_initialized(&mut self, interval: &Interval) {
        let (start_idx, end_idx) = self.visible_bounds(interval);
        self.visible_start_idx = start_idx;
        self.visible_end_idx = end_idx;
        self.visible_bounds_initialized = true;
    }

    pub fn rebuild_visible_for_interval(&mut self, interval: &Interval) {
        self.reset_mappings();
        let (start_idx, end_idx) = self.visible_bounds(interval);
        for pos in start_idx..end_idx {
            let log_index = self.logs_by_start[pos];
            if interval.contains_log(&self.logs[log_index]) {
                self.map_log(log_index);
            }
        }
        self.visible_start_idx = start_idx;
        self.visible_end_idx = end_idx;
        self.visible_bounds_initialized = true;
        self.sort();
    }

    pub fn reconcile_visible_for_interval(&mut self, interval: &Interval) {
        if !self.visible_bounds_initialized {
            self.rebuild_visible_for_interval(interval);
            return;
        }

        let old_start = self.visible_start_idx.min(self.logs.len());
        let old_end = self.visible_end_idx.min(self.logs.len());
        let (new_start, new_end) = self.visible_bounds(interval);

        if old_end > new_end {
            let start = old_start.max(new_end);
            for pos in start..old_end {
                self.unmap_log(self.logs_by_start[pos]);
            }
        }
        if new_start > old_start {
            let end = old_end.min(new_start);
            for pos in old_start..end {
                self.unmap_log(self.logs_by_start[pos]);
            }
        }

        if old_start > new_start {
            let end = old_start.min(new_end);
            for pos in new_start..end {
                let log_index = self.logs_by_start[pos];
                if interval.contains_log(&self.logs[log_index]) {
                    self.map_log(log_index);
                }
            }
        }
        if new_end > old_end {
            let start = old_end.max(new_start);
            for pos in start..new_end {
                let log_index = self.logs_by_start[pos];
                if interval.contains_log(&self.logs[log_index]) {
                    self.map_log(log_index);
                }
            }
        }

        self.visible_start_idx = new_start;
        self.visible_end_idx = new_end;
        self.visible_bounds_initialized = true;
        self.sort();
    }
}
