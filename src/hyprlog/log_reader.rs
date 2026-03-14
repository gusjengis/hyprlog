use anyhow::{Context, Result};
use chrono::TimeDelta;
use csv::{Reader, StringRecord};
use directories::BaseDirs;
use std::collections::BTreeSet;
use std::{
    fs::{create_dir_all, File},
    path::PathBuf,
};

use crate::{Interval, Settings};

pub struct LogReader {
    files: Vec<PathBuf>,       // absolute paths for each day, oldest → newest
    file_idx: usize,           // which file we’re on
    rdr: Option<Reader<File>>, // current csv reader
    last_headers: Option<StringRecord>,
    interval: Interval,
}

impl LogReader {
    pub fn new(settings: &Settings) -> Self {
        let base_dir = BaseDirs::new()
            .map(|b| b.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("hyprlog");

        create_dir_all(&base_dir).expect("failed to create data directory");
        // XOR focused + loaded intervals, then keep only the parts inside focused.
        let intervals_to_read: Vec<Interval> =
            interval_xor(&settings.focused_interval, &settings.loaded_interval)
                .into_iter()
                .filter_map(|interval| interval.overlap(&settings.focused_interval))
                .collect();

        let mut files_set = BTreeSet::new();
        for interval in intervals_to_read {
            let start = interval.start.date_naive();
            let end = interval.end.date_naive();

            let mut current = start;
            while current <= end {
                files_set.insert(base_dir.join(format!("{}.csv", current.format("%Y-%m-%d"))));
                current += TimeDelta::days(1);
            }
        }

        let mut files: Vec<PathBuf> = files_set.into_iter().collect();

        // skip non-existent files
        files = files.into_iter().filter(|p| p.exists()).collect();

        Self {
            files,
            file_idx: 0,
            rdr: None,
            last_headers: None,
            interval: settings.focused_interval.clone(),
        }
    }

    fn open_current(&mut self) -> Result<()> {
        if self.file_idx >= self.files.len() {
            self.rdr = None;
            return Ok(());
        }
        let path = &self.files[self.file_idx];
        let file = File::open(path)
            .with_context(|| format!("failed to open log file {}", path.to_string_lossy()))?;

        let mut rdr = csv::ReaderBuilder::new()
            .has_headers(true)
            .from_reader(file);

        // Stash/validate headers if you care that they’re consistent:
        let headers = rdr.headers().context("failed to read CSV headers")?.clone();

        if let Some(prev) = &self.last_headers {
            if prev != &headers {
                // Not fatal; warn or error. Here we just overwrite to keep going.
                // eprintln!("Warning: header mismatch in {}", path.to_string_lossy());
            }
        }
        self.last_headers = Some(headers);

        self.rdr = Some(rdr);
        Ok(())
    }

    /// Advance to next file; returns false if no more files.
    fn advance_file(&mut self) -> Result<bool> {
        self.file_idx += 1;
        if self.file_idx >= self.files.len() {
            self.rdr = None;
            Ok(false)
        } else {
            self.open_current()?;
            Ok(true)
        }
    }

    pub fn reset(&mut self) -> Result<()> {
        self.file_idx = 0;
        self.rdr = None;
        self.open_current()?;
        Ok(())
    }

    fn next_record(&mut self) -> Option<Result<StringRecord>> {
        loop {
            if self.rdr.is_none() {
                // Opening the very first file or after reset/advance.
                if self.file_idx >= self.files.len() {
                    return None;
                }
                if let Err(e) = self.open_current() {
                    // Skip unreadable file and try next
                    if self.advance_file().ok()? == false {
                        return Some(Err(e));
                    }
                    continue;
                }
            }

            let rdr = self.rdr.as_mut().unwrap();
            let mut rec = StringRecord::new();
            match rdr.read_record(&mut rec) {
                Ok(true) => {
                    let rec = sanitize_record(&rec);
                    let timestamp = rec.get(0).and_then(|field| field.parse::<u64>().ok());

                    if let Some(timestamp) = timestamp {
                        if self.interval.contains_utc_timestamp_millis(timestamp) {
                            return Some(Ok(rec));
                        } else {
                            continue;
                        }
                    } else {
                        continue;
                    }
                }
                Ok(false) => {
                    // End of this file; move to next file.
                    if let Err(e) = self.advance_file() {
                        return Some(Err(e));
                    }
                    if self.rdr.is_none() {
                        return None; // no more files
                    }
                    continue;
                }
                Err(err) => return Some(Err(err.into())),
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

fn sanitize_record(record: &StringRecord) -> StringRecord {
    StringRecord::from(
        record
            .iter()
            .map(|field| field.chars().filter(|&ch| ch != '\0').collect::<String>())
            .collect::<Vec<_>>(),
    )
}

fn interval_xor(a: &Interval, b: &Interval) -> Vec<Interval> {
    if let Some(overlap) = a.overlap(b) {
        let mut intervals = Vec::new();

        if a.start.min(b.start) < overlap.start {
            intervals.push(Interval {
                start: a.start.min(b.start),
                end: overlap.start,
                changed: false,
            });
        }

        if overlap.end < a.end.max(b.end) {
            intervals.push(Interval {
                start: overlap.end,
                end: a.end.max(b.end),
                changed: false,
            });
        }

        intervals
    } else {
        vec![a.clone(), b.clone()]
    }
}

/// Implement Iterator so you can `for rec in &mut reader { ... }`
impl Iterator for LogReader {
    type Item = Result<StringRecord>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_record()
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_record;
    use csv::StringRecord;

    #[test]
    fn strips_nuls_from_all_fields() {
        let record = StringRecord::from(vec![
            "\01773280161088",
            "SYST\0EM",
            "bo\0ot",
        ]);

        let sanitized = sanitize_record(&record);

        assert_eq!(sanitized.get(0), Some("1773280161088"));
        assert_eq!(sanitized.get(1), Some("SYSTEM"));
        assert_eq!(sanitized.get(2), Some("boot"));
    }
}
