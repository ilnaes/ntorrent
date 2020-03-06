use crate::client::Progress;
use crate::err::ConnectError;
use crate::torrents;
use crate::messages::{Message, Handshake, MessageID};
use crate::consts;
use crate::bitfield;
use crate::client;
use crate::queue::WorkQueue;
use crate::utils::calc_request;
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

    current: Option<torrents::Piece>,
    requested: usize,
    received: usize,
}

enum State {
    Entry,
    Choked,
    Unchoked
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
            current: None,
            requested: 0,
            received: 0,
        }
    }

    async fn send_message(&mut self, m: Message) -> Result<(), Box<dyn Error>> {
        if let Some(s) = &mut self.stream {
            timeout(consts::TIMEOUT, s.write_all(m.serialize().as_slice())).await??;
            return Ok(())
        }
        Err(Box::new(ConnectError))
    }

    async fn read_message(&mut self) -> Result<Message, Box<dyn Error>> {
        if let Some(s) = &mut self.stream {
            return Message::read_from(s).await
        }
        Err(Box::new(ConnectError))
    }

    // tries to connect to ip and handshake (with timeouts)
    async fn handshake(&self, ip: String) -> Result<TcpStream, Box<dyn Error>> {
        let mut s = timeout(consts::TIMEOUT, TcpStream::connect(ip)).await??;

        let mut buf = [0; 128];
        timeout(consts::TIMEOUT, s.write_all(self.handshake.as_slice())).await??;
        timeout(consts::TIMEOUT, s.read(&mut buf)).await??;

        if let Some(h) = Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.info_hash {
                return Err(Box::new(ConnectError))
            }
        }
        Ok(s)
    }

    // repeatedly attempts to connect to a peer, handshake
    // and get bitfield until success
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

            println!("Worker {} attempting to connect to {}", self.id, ip);
            let res = self.handshake(ip.clone()).await.ok();

            if let Some(s) = res {
                self.stream = Some(s);
                // TODO: send bitfield
                return;
            }
            println!("Worker {} not connected", self.id);
            // put ip back
            self.peers.push(ip).await;
        }
    }

    // sends requests and pieces and chooses new work piece if necessary
    // returns false if no more work to be done
    async fn manage_io(&mut self, bf: &bitfield::Bitfield) -> bool {
        if self.current == None {
            self.current = self.work.find_first(|x| bf.has(x.1)).await;
            self.requested = 0;
            if self.current == None {
                // no more work
                return false
            }
        }

        if let Some(piece) = self.current {
            while self.requested < self.received + consts::MAXREQUESTS && self.requested * consts::BLOCKSIZE< piece.2 {
                if let Ok(msg) = calc_request(piece.1, self.requested, piece.2) {
                    let res = self.send_message(Message {
                        message_id: MessageID::Request,
                        payload: Some(msg),
                    }).await.ok();

                    if res == None {
                        // error
                        return false
                    }
                    self.requested += 1;
                } else {
                    // error
                    return false
                }
            }
        }

        true
    }

    async fn process_piece(&mut self, piece: Vec<u8>) {
    }

    pub async fn download(&mut self) {
        self.connect().await;
        println!("Worker {} connected", self.id);

        let mut state = State::Entry;
        let mut bf = bitfield::Bitfield {
            bf: Vec::new(),
        };

        loop {
            let response = self.read_message().await.ok();
            if let Some(msg) = response {
                match state {
                    State::Entry => {
                        if msg.message_id == MessageID::Bitfield && msg.payload != None {
                            bf.bf = msg.payload.unwrap();

                            // find work if exists and send interested
                            self.current = self.work.find_first(|x| bf.has(x.1)).await;
                            println!("Worker {} attemping to download piece {:?}", self.id, self.current);

                            if self.current != None {
                                let res = self.send_message(Message{
                                    message_id: MessageID::Interested,
                                    payload: None,
                                }).await.ok();

                                if res == None {
                                    break
                                }
                                state = State::Choked;
                            } else {
                                break
                            }
                        }
                    },

                    State::Choked => {
                        match msg.message_id {
                            MessageID::Unchoke => {
                                println!("Worker {} unchoked!", self.id);
                                state = State::Unchoked;
                                if !self.manage_io(&bf).await {
                                    break
                                }
                            },
                            MessageID::Piece => {
                                self.process_piece(msg.payload.unwrap()).await;
                            },
                            _ => ()
                        }
                    },

                    State::Unchoked => {
                        match msg.message_id {
                            MessageID::Choke => {
                                println!("Worker {} choked!", self.id);
                                state = State::Choked;
                            },
                            MessageID::Piece => {
                                self.process_piece(msg.payload.unwrap()).await;
                                if !self.manage_io(&bf).await {
                                    break
                                }
                            },
                            _ => (),
                        }
                    },
                }
            } else {
                println!("Worker {} timed out reading", self.id);
                break
            }
        }

        println!("Worker {} breaking off", self.id);

        // if there is still work, return it to queue
        if let Some(piece) = self.current.take() {
            self.work.push(piece).await;
        }
    }
}
