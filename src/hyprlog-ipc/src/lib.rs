use postcard::{from_bytes, to_stdvec};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum DaemonToClient {
    Welcome {
        current_seq: u64,
    },
    Log {
        seq: u64,
        timestamp: i64,
        class: String,
        title: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientToDaemon {
    Subscribe { client_id: Uuid },
    Unsubscribe { client_id: Uuid },
}

impl DaemonToClient {
    pub fn encode(&self) -> Vec<u8> {
        to_stdvec(self).unwrap()
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        from_bytes(buf).ok()
    }
}

impl ClientToDaemon {
    pub fn encode(&self) -> Vec<u8> {
        to_stdvec(self).unwrap()
    }

    pub fn decode(buf: &[u8]) -> Option<Self> {
        from_bytes(buf).ok()
    }
}
