use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::runtime::Runtime;
use uuid::Uuid;

use hyprlog_ipc::{ClientToDaemon, DaemonToClient};

pub enum StreamEvent {
    Log {
        seq: u64,
        timestamp: i64,
        class: String,
        title: String,
    },
    Welcome {
        current_seq: u64,
    },
    #[allow(dead_code)]
    Gap {
        expected: u64,
        received: u64,
    },
    Disconnected,
}

struct StreamState {
    events: Vec<StreamEvent>,
}

pub struct StreamClient {
    state: Arc<Mutex<StreamState>>,
    force_render: Arc<AtomicBool>,
    _join_handle: thread::JoinHandle<()>,
}

impl StreamClient {
    pub fn connect() -> std::io::Result<Self> {
        let state = Arc::new(Mutex::new(StreamState { events: Vec::new() }));
        let force_render = Arc::new(AtomicBool::new(false));
        let client_id = Uuid::new_v4();
        let state_clone = Arc::clone(&state);
        // let force_render_clone = Arc::clone(&force_render);

        let join_handle = thread::spawn(move || {
            let rt = Runtime::new().unwrap();
            let state_for_error = Arc::clone(&state_clone);
            rt.block_on(async move {
                if let Err(e) = run_client(state_clone, client_id).await {
                    let mut state = state_for_error.lock().unwrap();
                    state.events.push(StreamEvent::Disconnected);
                    eprintln!("Stream client error: {}", e);
                }
            });
        });

        Ok(Self {
            state,
            force_render,
            _join_handle: join_handle,
        })
    }

    pub fn try_recv(&mut self) -> Result<StreamEvent, std::sync::mpsc::TryRecvError> {
        let mut state = self.state.lock().unwrap();
        if state.events.is_empty() {
            return Err(std::sync::mpsc::TryRecvError::Empty);
        }
        let event = state.events.remove(0);

        if matches!(event, StreamEvent::Log { .. }) {
            self.force_render.store(true, Ordering::SeqCst);
        }

        Ok(event)
    }

    pub fn force_render(&self) -> &Arc<AtomicBool> {
        &self.force_render
    }
}

async fn run_client(state: Arc<Mutex<StreamState>>, client_id: Uuid) -> std::io::Result<()> {
    const SOCKET_PATH: &str = "/tmp/hyprlog-stream.sock";

    let mut stream = UnixStream::connect(SOCKET_PATH).await?;

    let subscribe_msg = ClientToDaemon::Subscribe { client_id };
    stream.write_all(&subscribe_msg.encode()).await?;

    let mut buf = [0u8; 512];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break;
        }

        if let Some(msg) = DaemonToClient::decode(&buf[..n]) {
            let event = match msg {
                DaemonToClient::Welcome { current_seq } => StreamEvent::Welcome { current_seq },
                DaemonToClient::Log {
                    seq,
                    timestamp,
                    class,
                    title,
                } => StreamEvent::Log {
                    seq,
                    timestamp,
                    class,
                    title,
                },
            };
            let mut state = state.lock().unwrap();
            state.events.push(event);
        }
    }

    Ok(())
}
