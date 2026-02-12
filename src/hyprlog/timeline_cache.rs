use std::collections::HashMap;

const MAX_ENTRIES_PER_KEY: usize = 64;

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

    pub fn exact_entry(
        &self,
        key: &TimelineCacheKey,
        start_ms: u64,
        end_ms: u64,
    ) -> Option<&TimelineCacheEntry> {
        self.entries
            .get(key)?
            .iter()
            .rev()
            .find(|entry| entry.start_ms == start_ms && entry.end_ms == end_ms)
    }

    pub fn insert(&mut self, key: TimelineCacheKey, entry: TimelineCacheEntry) {
        if entry.end_ms <= entry.start_ms {
            return;
        }

        let bucket = self.entries.entry(key.clone()).or_default();

        if let Some(pos) = bucket
            .iter()
            .position(|e| e.start_ms == entry.start_ms && e.end_ms == entry.end_ms)
        {
            bucket[pos] = entry;
            return;
        }

        bucket.push(entry);
        bucket.sort_by_key(|e| e.start_ms);

        let mut i = 0usize;
        while i + 1 < bucket.len() {
            let can_merge = bucket[i].end_ms >= bucket[i + 1].start_ms;
            if can_merge {
                let right = bucket.remove(i + 1);
                let left = bucket.remove(i);
                let merged = merge_entries(&key, left, right);
                bucket.insert(i, merged);
                i = i.saturating_sub(1);
            } else {
                i += 1;
            }
        }

        if bucket.len() > MAX_ENTRIES_PER_KEY {
            let keep_from = bucket.len() - MAX_ENTRIES_PER_KEY;
            bucket.drain(0..keep_from);
        }
    }
}

fn merge_entries(
    key: &TimelineCacheKey,
    left: TimelineCacheEntry,
    right: TimelineCacheEntry,
) -> TimelineCacheEntry {
    let ms = key.ms_per_character.max(1);
    let start_ms = left.start_ms.min(right.start_ms);
    let end_ms = left.end_ms.max(right.end_ms);
    let total_cols = ((end_ms - start_ms) / ms) as usize;

    let mut timeline = vec![' '; total_cols];
    let mut labels = vec![String::new(); total_cols];

    let left_chars: Vec<char> = left.timeline.chars().collect();
    let right_chars: Vec<char> = right.timeline.chars().collect();

    let left_off = ((left.start_ms - start_ms) / ms) as usize;
    for (i, ch) in left_chars.iter().enumerate() {
        if left_off + i < timeline.len() {
            timeline[left_off + i] = *ch;
        }
    }
    for (i, lbl) in left.section_labels.iter().enumerate() {
        if left_off + i < labels.len() {
            labels[left_off + i] = lbl.clone();
        }
    }

    let right_off = ((right.start_ms - start_ms) / ms) as usize;
    for (i, ch) in right_chars.iter().enumerate() {
        if right_off + i < timeline.len() {
            timeline[right_off + i] = *ch;
        }
    }
    for (i, lbl) in right.section_labels.iter().enumerate() {
        if right_off + i < labels.len() {
            labels[right_off + i] = lbl.clone();
        }
    }

    TimelineCacheEntry {
        start_ms,
        end_ms,
        timeline: timeline.into_iter().collect(),
        section_labels: labels,
    }
}
