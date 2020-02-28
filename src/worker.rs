use crate::client::Progress;
use crate::err::ConnectError;
use crate::torrents;
use crate::messages;
use crate::consts;
use crate::bitfield;
use crate::client;
use crate::queue::WorkQueue;
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
    work: WorkQueue<torrents::Piece>,
    handshake: Vec<u8>,
    info_hash: Vec<u8>,
    stream: Option<TcpStream>,
}

enum State {
    Choked
}

impl Worker {
    pub fn from_client(c: &client::Client, tx: mpsc::Sender<Vec<u8>>, i: u64) -> Worker {
        Worker {
            id: i+1,
            tx,
            progress: Arc::clone(&c.progress),
            peers: c.peer_list.clone(),
            work: c.torrent.pieces.clone(),
            handshake: c.handshake.clone(),
            info_hash: c.torrent.info_hash.clone(),
            stream: None,
        }
    }

    async fn send_message(&mut self, m: messages::Message) -> Result<(), Box<dyn Error>> {
        if let Some(s) = &mut self.stream {
            timeout(consts::TIMEOUT, s.write_all(m.serialize().as_slice())).await??;
            return Ok(())
        }
        Err(Box::new(ConnectError))
    }

    async fn read_message(&mut self) -> Result<messages::Message, Box<dyn Error>> {
        if let Some(s) = &mut self.stream {
            return messages::Message::read_from(s).await;
        }
        Err(Box::new(ConnectError))
    }

    // tries to connect to ip and handshake (with timeouts)
    async fn handshake(&self, ip: String) -> Result<TcpStream, Box<dyn Error>> {
        let mut s = timeout(consts::TIMEOUT, TcpStream::connect(ip)).await??;

        let mut buf = [0; 128];
        timeout(consts::TIMEOUT, s.write_all(self.handshake.as_slice())).await??;
        timeout(consts::TIMEOUT, s.read(&mut buf)).await??;

        if let Some(h) = messages::Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.info_hash {
                return Err(Box::new(ConnectError))
            }
        }
        Ok(s)
    }

    // repeatedly attempts to connect to a peer, handshake
    // and get bitfield until success
    async fn connect(&mut self) -> bitfield::Bitfield {
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

            println!("Worker {} attempting to connect to {}", self.id, ip);
            let res = self.handshake(ip.clone()).await.ok();

            if let Some(s) = res {
                self.stream = Some(s);

                // get bitfield
                let msg = self.read_message().await;
                if let Ok(m) = msg {
                    if m.message_id == messages::MessageID::Bitfield && m.payload != None {
                        // TODO: verify bitfield
                        return bitfield::Bitfield {
                            bf: m.payload.unwrap(),
                        }
                    }
                }
            }
            println!("Worker {} not connected", self.id);
            // put ip back
            self.peers.push(ip).await;
        }
    }


    // uses small state machine to download piece
    // starting state: choked with no downloads
    pub async fn download_piece(&mut self, piece: torrents::Piece) -> bool {
        let mut state = State::Choked;
        loop {
            return false
        }
    }
    
    pub async fn download(&mut self) {
        let bf = self.connect().await;
        println!("Worker {} got bitfield", self.id);

        let mut interested = false;
        loop {
            // find work if exists
            let possible = self.work.find_first(|x| {
                bf.has(x.1)
            }).await;

            if let Some(piece) = possible {
                // send interested message
                let response = if !interested {
                    let res = self.send_message(messages::Message {
                        message_id: messages::MessageID::Interested,
                        payload: None,
                    }).await.ok();

                    if let Some(_) = res {
                        interested = true;
                    }
                    res
                } else {
                    Some(())
                };

                if let Some(_) = response {
                    // start state machine downloader
                    self.download_piece(piece).await;
                }
            } else {
                break
            }
        }
    }
}
