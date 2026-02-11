use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineMode {
    SingleAll,
    SingleAllFull,
    SingleClass,
    SingleClassFull,
    MultiClass,
    MultiTitle,
    MultiFull,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TimelineCacheKey {
    pub ms_per_character: u64,
    pub mode: TimelineMode,
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub struct TimelineCacheEntry {
    pub start_ms: u64,
    pub end_ms: u64,
    pub timeline: String,
    pub section_labels: Vec<String>,
}

#[derive(Default)]
pub struct TimelineCache {
    entries: HashMap<TimelineCacheKey, Vec<TimelineCacheEntry>>,
}

impl TimelineCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn entries_for_key(&self, key: &TimelineCacheKey) -> Option<&Vec<TimelineCacheEntry>> {
        self.entries.get(key)
    }

    pub fn insert(&mut self, key: TimelineCacheKey, entry: TimelineCacheEntry) {
        self.entries.entry(key).or_default().push(entry);
    }
}
