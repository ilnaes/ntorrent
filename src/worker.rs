use crate::client::Progress;
use crate::downloader;
use crate::torrents;
use crate::messages;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpStream;
use tokio::prelude::*;
use std::time::Duration;
use tokio::time::timeout;
use std::error::Error;

pub struct Worker {
    tx: mpsc::Sender<Vec<u8>>,
    progress: Arc<Mutex<Progress>>,
    peers: Arc<Mutex<VecDeque<String>>>,
    work: Arc<Mutex<VecDeque<torrents::Piece>>>,
    handshake: Vec<u8>,
    info_hash: Vec<u8>,
}

impl Worker {
    pub fn from(mgr: &downloader::Manager, tx: mpsc::Sender<Vec<u8>>) -> Worker {
        Worker {
            tx,
            progress: Arc::clone(&mgr.progress),
            peers: Arc::clone(&mgr.peer_list),
            work: Arc::clone(&mgr.pieces),
            handshake: mgr.handshake.clone(),
            info_hash: mgr.info_hash.clone(),
        }
    }

    // tries to connect to ip and handshake (with timeouts)
    pub async fn handshake(&self, ip: String) -> Result<TcpStream, Box<dyn Error>> {
        let mut s = timeout(Duration::from_secs(5), TcpStream::connect(ip)).await??;

        let mut buf = [0; 128];
        timeout(Duration::from_secs(5), s.write_all(self.handshake.as_slice())).await??;
        timeout(Duration::from_secs(5), s.read(&mut buf)).await??;

        if let Some(h) = messages::Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.info_hash {
                return Err(Box::new(std::io::Error::from_raw_os_error(22)))
            }
        }
        Ok(s)

    }

    // repeatedly attempts to connect to a peer and handshake until success
    pub async fn connect(&self) -> TcpStream {
        loop {
            let ip;
            {
                loop {
                    if let Some(s) = self.peers.lock().await.pop_front() {
                        ip = s;
                        break
                    } else {
                        // wait 1s before getting another peer
                        tokio::time::delay_for(Duration::from_secs(1)).await;
                    }
                }
            }

            if let Ok(s) = self.handshake(ip).await {
                return s
            }
        }
    }
    
    pub async fn download(&self) {
        loop {
            let mut stream = self.connect().await;
        }
    }
}
