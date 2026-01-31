mod log_writer;
mod shutdown;
mod socket;
mod stream_server;

use std::{env, path::Path, sync::Arc, time::Duration};

use hyprland::event_listener::{AsyncEventListener, WindowEventData};
use log_writer::{log_error, run_log_writer, LogMsg};
use shutdown::{try_spawn_logind_shutdown_watcher, wait_for_shutdown_signal};
use socket::start_socket_listener;
use stream_server::StreamServer;
use tokio::sync::mpsc;

const STREAM_SOCKET_PATH: &str = "/tmp/hyprlog-stream.sock";

#[tokio::main]
async fn main() -> hyprland::Result<()> {
    let mut settings = Settings::new();

    let args: Vec<String> = env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("snitch") => settings.snitch = true,
        _ => {}
    };

    let (sender_handle, receiver_handle) = mpsc::channel::<LogMsg>(1024);

    let stream_server = Arc::new(StreamServer::new(Path::new(STREAM_SOCKET_PATH))?);
    let broadcaster = {
        let server = Arc::clone(&stream_server);
        move |ts: i64, class: String, title: String| {
            server.broadcast(ts, class, title);
        }
    };

    let writer_jh = tokio::spawn(run_log_writer(receiver_handle, settings, broadcaster));

    let stream_server_run = Arc::clone(&stream_server);
    let _stream_jh = tokio::spawn(async move {
        stream_server_run.run().await;
    });

    let _ = sender_handle
        .send(LogMsg::Line {
            ts: chrono::Utc::now().timestamp_millis(),
            class: "SYSTEM".into(),
            title: "boot".into(),
        })
        .await;

    {
        let sender_handle_static: &'static mpsc::Sender<LogMsg> =
            Box::leak(Box::new(sender_handle.clone()));

        tokio::spawn(async move {
            loop {
                let mut event_listener = AsyncEventListener::new();

                #[allow(deprecated)]
                {
                    event_listener.add_active_window_changed_handler(
                    hyprland::prelude::async_closure! { move |window_data: Option<WindowEventData>| {
                                if let Some(ref data) = window_data {
                                    let class = data.class.clone();
                                    let title = data.title.clone();

                                    let _ = sender_handle_static.try_send(LogMsg::Line {
                                        ts: chrono::Utc::now().timestamp_millis(),
                                        class,
                                        title,
                                    });
                                }
                            }
                        },
                    );
                }
                if let Err(e) = event_listener.start_listener_async().await {
                    let ts = chrono::Utc::now().timestamp_millis();
                    log_error(format!("{ts}, [hypr] listener ended: {e}; retrying in 1s"));
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        });
    }

    {
        let sender_handle_sock = sender_handle.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = start_socket_listener(sender_handle_sock.clone()).await {
                    let ts = chrono::Utc::now().timestamp_millis();
                    log_error(format!("{ts}, [sock] listener failed: {e}; retrying in 3s",));
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
            }
        });
    }

    if let Some(jhandle) = try_spawn_logind_shutdown_watcher(sender_handle.clone()).await {
        let _ = jhandle.await;
    } else {
        wait_for_shutdown_signal().await;

        let _ = sender_handle
            .send(LogMsg::Line {
                ts: chrono::Utc::now().timestamp_millis(),
                class: String::from("SYSTEM"),
                title: String::from("shutdown"),
            })
            .await;
    }

    drop(sender_handle);
    let _ = writer_jh.await;

    Ok(())
}

#[derive(Clone)]
pub struct Settings {
    pub snitch: bool,
}

impl Settings {
    fn new() -> Self {
        Self { snitch: false }
    }
}
