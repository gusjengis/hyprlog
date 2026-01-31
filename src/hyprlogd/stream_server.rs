use std::fs::remove_file;
use std::{
    collections::HashMap,
    path::Path,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use uuid::Uuid;

use hyprlog_ipc::{ClientToDaemon, DaemonToClient};

pub struct StreamServer {
    listener: UnixListener,
    connections: Arc<Mutex<HashMap<Uuid, Connection>>>,
    next_seq: Arc<AtomicU64>,
}

struct Connection {
    stream: UnixStream,
    last_seq: u64,
}

impl StreamServer {
    pub fn new(path: &Path) -> std::io::Result<Self> {
        if path.exists() {
            remove_file(path)?;
        }
        let listener = UnixListener::bind(path)?;

        Ok(Self {
            listener,
            connections: Arc::new(Mutex::new(HashMap::new())),
            next_seq: Arc::new(AtomicU64::new(0)),
        })
    }

    pub async fn run(self: Arc<Self>) {
        loop {
            match self.listener.accept().await {
                Ok((stream, _)) => {
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        handle_connection(stream, server).await;
                    });
                }
                Err(e) => {
                    eprintln!("Stream server accept error: {}", e);
                }
            }
        }
    }

    pub fn broadcast(&self, timestamp: i64, class: String, title: String) {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let msg = DaemonToClient::Log {
            seq,
            timestamp,
            class,
            title,
        };
        let encoded = msg.encode();

        let mut connections = self.connections.lock().unwrap();
        connections.retain(|_id, conn| {
            if let Err(e) = conn.stream.try_write(&encoded) {
                eprintln!("Stream write error: {}", e);
                false
            } else {
                conn.last_seq = seq;
                true
            }
        });
    }

    pub fn current_seq(&self) -> u64 {
        self.next_seq.load(Ordering::SeqCst)
    }
}

async fn handle_connection(mut stream: UnixStream, server: Arc<StreamServer>) {
    let mut buf = [0u8; 512];
    let n = match stream.read(&mut buf).await {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Stream read error: {}", e);
            return;
        }
    };

    if n == 0 {
        return;
    }

    if let Some(ClientToDaemon::Subscribe { client_id }) = ClientToDaemon::decode(&buf[..n]) {
        let mut connections = server.connections.lock().unwrap();
        connections.insert(
            client_id,
            Connection {
                stream,
                last_seq: server.current_seq(),
            },
        );

        let welcome_msg = DaemonToClient::Welcome {
            current_seq: server.current_seq(),
        };
        if let Err(e) = connections
            .get(&client_id)
            .unwrap()
            .stream
            .try_write(&welcome_msg.encode())
        {
            eprintln!("Welcome write error: {}", e);
            connections.remove(&client_id);
        }
    } else if let Some(ClientToDaemon::Unsubscribe { client_id }) =
        ClientToDaemon::decode(&buf[..n])
    {
        let mut connections = server.connections.lock().unwrap();
        connections.remove(&client_id);
    }
}
