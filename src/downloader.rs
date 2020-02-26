use crate::client::{Client, Progress};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::collections::VecDeque;
use crate::torrents;
use crate::worker::Worker;
use crate::queue::WorkQueue;

pub struct Manager {
    nworkers: u64,
    pub progress: Arc<Mutex<Progress>>,
    pub peer_list: WorkQueue<String>,
    pub pieces: Arc<Mutex<VecDeque<torrents::Piece>>>,
    pub handshake: Vec<u8>,
    pub info_hash: Vec<u8>,
}

impl Manager {
    pub fn from(c: &Client, nworkers: u64) -> Manager {
        Manager {
            nworkers,
            progress: Arc::clone(&c.progress),
            peer_list: c.peer_list.clone(),
            pieces: Arc::new(Mutex::new(c.torrent.pieces.clone())),
            handshake: c.handshake.clone(),
            info_hash: c.torrent.info_hash.clone(),
        }
    }

    pub async fn download(&self) {
        let n;
        {
            let pieces = self.pieces.lock().await;
            n = pieces.len();
        }
        let (tx, mut rx) = mpsc::channel(n);
        
        for i in 0..self.nworkers {
            let mut w = Worker::from(&self, tx.clone(), i);
            tokio::spawn(async move {
                w.download().await;
            });
        }
    }
}
