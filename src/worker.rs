use crate::client::Progress;
use crate::err::ConnectError;
use crate::downloader;
use crate::torrents;
use crate::messages;
use crate::consts;
use crate::queue::WorkQueue;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tokio::net::TcpStream;
use tokio::prelude::*;
use std::time::Duration;
use tokio::time::timeout;
use std::error::Error;

pub struct Worker {
    id: u64,
    tx: mpsc::Sender<Vec<u8>>,
    progress: Arc<Mutex<Progress>>,
    peers: WorkQueue<String>,
    work: Arc<Mutex<VecDeque<torrents::Piece>>>,
    handshake: Vec<u8>,
    info_hash: Vec<u8>,
    stream: Option<TcpStream>,
    bitfield: Vec<u8>,
}

impl Worker {
    pub fn from(mgr: &downloader::Manager, tx: mpsc::Sender<Vec<u8>>, i: u64) -> Worker {
        Worker {
            id: i+1,
            tx,
            progress: Arc::clone(&mgr.progress),
            peers: mgr.peer_list.clone(),
            work: Arc::clone(&mgr.pieces),
            handshake: mgr.handshake.clone(),
            info_hash: mgr.info_hash.clone(),
            stream: None,
            bitfield: Vec::new(),
        }
    }

    async fn read_message(&mut self) -> Result<messages::Message, Box<dyn Error>> {
        if let Some(s) = &mut self.stream {
            return messages::Message::read_from(s).await;
        }
        return Err(Box::new(ConnectError))
    }

    // tries to connect to ip and handshake (with timeouts)
    async fn handshake(&self, ip: String) -> Result<TcpStream, Box<dyn Error>> {
        println!("Worker {} attempting to connect to {}", self.id, ip);
        let mut s = timeout(consts::TIMEOUT, TcpStream::connect(ip)).await??;

        let mut buf = [0; 128];
        println!("Worker {} attempting to handshake", self.id);
        timeout(consts::TIMEOUT, s.write_all(self.handshake.as_slice())).await??;
        timeout(consts::TIMEOUT, s.read(&mut buf)).await??;

        if let Some(h) = messages::Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.info_hash {
                return Err(Box::new(ConnectError))
            }
        }
        Ok(s)
    }

    // repeatedly attempts to connect to a peer and handshake until success
    async fn connect(&mut self) {
        loop {
            let ip: String;
            {
                loop {
                    if let Some(s) = self.peers.pop().await {
                        ip = s;
                        break
                    } else {
                        // wait 1s before getting another peer
                        tokio::time::delay_for(Duration::from_secs(1)).await;
                    }
                }
            }

            let res = self.handshake(ip.clone()).await.ok();

            if let Some(s) = res {
                println!("Worker {} handshook", self.id);
                self.stream = Some(s);

                // get bitfield
                let msg = self.read_message().await;
                if let Ok(m) = msg {
                    if m.message_id == messages::MessageID::Bitfield {
                        println!("Worker {} got bitfield!", self.id);
                        self.bitfield = m.payload.unwrap();
                        break
                    }
                }
            }
            println!("Worker {} not connected", self.id);
            // put ip back
            self.peers.push(ip).await;
        }
    }

    async fn find_piece(&self) -> Option<torrents::Piece> {
        let mut workq = self.work.lock().await;

        None
    }
    
    pub async fn download(&mut self) {
        self.connect().await;
    }
}
