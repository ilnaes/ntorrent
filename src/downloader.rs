use crate::client::{Client, Progress};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

pub struct Manager {
    nworkers: u64,
    progress: Arc<Mutex<Progress>>,
    peer_list: Arc<Mutex<Vec<String>>>,
    info_hash: Vec<u8>,
    peer_id: Vec<u8>,
}

impl Manager {
    pub fn from(c: &Client, nworkers: u64) -> Manager {
        Manager {
            nworkers,
            progress: Arc::clone(&c.progress),
            peer_list: Arc::clone(&c.peer_list),
            info_hash: c.torrent.info_hash.clone(),
            peer_id: c.torrent.peer_id.clone(),
        }
    }
}
