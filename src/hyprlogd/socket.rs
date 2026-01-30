use std::{fs::remove_file, path::Path};

use tokio::sync::mpsc::Sender;

use crate::log_writer::{log_error, LogMsg};

use tokio::net::UnixDatagram;
pub async fn start_socket_listener(sender: Sender<LogMsg>) -> std::io::Result<()> {
    const SOCKET_PATH: &str = "/tmp/hyprlog.sock";
    if Path::new(SOCKET_PATH).exists() {
        let _ = remove_file(SOCKET_PATH);
    }
    let sock = UnixDatagram::bind(SOCKET_PATH)?; // no per-connection tasks

    let mut buf = vec![0u8; 256];
    loop {
        let (n, _addr) = sock.recv_from(&mut buf).await?;
        let cmd = std::str::from_utf8(&buf[..n]).unwrap_or("").trim();
        let ts = chrono::Utc::now().timestamp_millis();
        let (class, title) = match cmd {
            "idle" => ("SYSTEM", "idle"),
            "resume" => ("SYSTEM", "resume"),
            other => {
                log_error(format!("{ts}, Unknown message: {other}"));
                continue;
            }
        };
        let _ = sender
            .send(LogMsg::Line {
                ts,
                class: class.into(),
                title: title.into(),
            })
            .await;
    }
}
