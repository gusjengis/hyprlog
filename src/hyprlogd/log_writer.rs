use chrono::{Datelike, Utc};
use directories::BaseDirs;
use std::{
    fs::{create_dir_all, File, OpenOptions},
    io::{self, Write},
    os::unix::net::UnixDatagram,
    path::PathBuf,
    time::Duration,
};

use crate::Settings;
pub enum LogMsg {
    Line {
        ts: i64,
        class: String,
        title: String,
    },
    Shutdown,
}

struct LogWriter {
    base_dir: PathBuf,
    day_key: (i32, u32, u32), // (year, month, day)
    file: File,
    settings: Settings,
}

impl LogWriter {
    fn init(base_dir: PathBuf, settings: Settings) -> io::Result<Self> {
        create_dir_all(&base_dir)?;
        let (day_key, path) = Self::today_path(&base_dir);
        let file = LogWriter::create_log_file(&path)?;
        Ok(Self {
            base_dir,
            day_key,
            file,
            settings,
        })
    }

    fn today_path(base_dir: &PathBuf) -> ((i32, u32, u32), PathBuf) {
        let now = Utc::now();
        let day_key = (now.year(), now.month(), now.day());
        let path = base_dir.join(format!("{}", now.format("%Y-%m-%d.csv")));
        (day_key, path)
    }

    fn ensure_today(&mut self) -> io::Result<()> {
        let (today_key, path) = Self::today_path(&self.base_dir);
        if today_key != self.day_key {
            self.file = LogWriter::create_log_file(&path)?;
            self.day_key = today_key;
        }
        Ok(())
    }

    fn write_line(&mut self, ts: i64, class: &str, title: &str) -> io::Result<()> {
        self.ensure_today()?;
        let safe_title = title.replace('"', "\"\"");
        let line = format!("{},{},\"{}\"\n", ts, class, safe_title);
        self.file.write_all(line.as_bytes())?;
        if self.settings.snitch {
            send_datagram(line.as_str());
        }

        Ok(())
    }

    fn create_log_file(path: &PathBuf) -> io::Result<File> {
        let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
        if f.metadata()?.len() == 0 {
            f.write_all(b"timestamp,class,title\n")?;
        }

        Ok(f)
    }
}

pub async fn run_log_writer(
    mut receiver_handle: tokio::sync::mpsc::Receiver<LogMsg>,
    settings: Settings,
) {
    let base_dir = BaseDirs::new()
        .map(|b| b.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("hyprlog");

    let mut writer = loop {
        match LogWriter::init(base_dir.clone(), settings.clone()) {
            Ok(w) => break w,
            Err(e) => {
                let ts = chrono::Utc::now().timestamp_millis();
                log_error(format!("{ts}, [writer] init failed: {e}; retrying in 1s")); // output to file
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    };

    //  listen for LogMsg(s) and write them to todays log file, that basically sums up this whole file
    while let Some(msg) = receiver_handle.recv().await {
        match msg {
            LogMsg::Line { ts, class, title } => {
                if let Err(e) = writer.write_line(ts, &class, &title) {
                    log_error(format!("{ts}, [writer] write failed: {e}; retry in 500ms"));
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    if let Err(e2) = writer.write_line(ts, &class, &title) {
                        log_error(format!(
                            "{ts}, [writer] write failed again: {e2}; dropping line"
                        ));
                    }
                }
            }
            LogMsg::Shutdown => {
                let _ = writer.file.flush();
                break;
            }
        }
    }
}

pub fn log_error<S: AsRef<str>>(text: S) {
    match {
        let dir = BaseDirs::new()
            .map(|b| b.data_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("hyprlog");
        create_dir_all(&dir).unwrap();

        let log_path = dir.join("hyprlogd.log");
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .unwrap();

        eprintln!("{}", text.as_ref());
        writeln!(file, "{}", text.as_ref())
    } {
        Ok(_) => {}
        Err(e) => eprintln!("Failed to write error to log.\n\n{e}"),
    };
}

fn send_datagram(msg: &str) {
    const SOCKET_PATH: &str = "/tmp/hyprlog-snitch.sock";
    if let Ok(sock) = UnixDatagram::unbound() {
        let _ = sock.send_to(msg.as_bytes(), SOCKET_PATH);
    }
}
