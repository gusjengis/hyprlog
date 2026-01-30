use anyhow::{Context, Result};
use chrono::TimeDelta;
use csv::{Reader, StringRecord};
use directories::BaseDirs;
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
        // get all files in the interval
        let start = settings.interval.start.date_naive();
        let end = settings.interval.end.date_naive();

        let mut files = Vec::new();
        let mut current = start;

        while current <= end {
            files.push(base_dir.join(format!("{}.csv", current.format("%Y-%m-%d"))));
            current += TimeDelta::days(1);
        }

        // skip non-existent files
        files = files.into_iter().filter(|p| p.exists()).collect();

        Self {
            files,
            file_idx: 0,
            rdr: None,
            last_headers: None,
            interval: settings.interval.clone(),
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
                    if self
                        .interval
                        .contains_utc_timestamp_millis(rec[0].parse::<u64>().unwrap())
                    {
                        return Some(Ok(rec));
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

/// Implement Iterator so you can `for rec in &mut reader { ... }`
impl Iterator for LogReader {
    type Item = Result<StringRecord>;
    fn next(&mut self) -> Option<Self::Item> {
        self.next_record()
    }
}
