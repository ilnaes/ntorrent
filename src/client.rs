use crate::messages;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::queue::WorkQueue;
use crate::worker::Worker;
use tokio::sync::{Mutex, mpsc};
use std::sync::Arc;

pub struct Progress {
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
}

pub struct Client {
    nworkers: u64,
    pub torrent: Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: WorkQueue<String>,
    pub progress: Arc<Mutex<Progress>>,
    pub port: i64,
}

impl Client {
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        let left = torrent.files.iter().map(|x| x.length).fold(0, |a,b| a+b);
        let handshake = messages::Handshake::from(&torrent).serialize();

        Client {
            nworkers: 4,
            torrent,
            peer_list: WorkQueue::new(),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                left
            })),
            port: 2222,
            handshake,
        }
    }

    pub async fn manage_workers(&self) {
        let n = self.torrent.pieces.len().await;
        let (tx, mut rx) = mpsc::channel(n);
        
        for i in 0..self.nworkers {
            let mut w = Worker::from_client(&self, tx.clone(), i);
            tokio::spawn(async move {
                w.download().await;
            });
        }
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        tokio::join!(peerlist.poll_peerlist(), self.manage_workers());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn peerlist_test() {}
}
