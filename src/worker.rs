use crate::client::Progress;
use crate::err::ConnectError;
use crate::torrents;
use crate::messages;
use crate::messages::{Message, Handshake, MessageID};
use crate::consts;
use crate::bitfield;
use crate::client;
use crate::opstream::OpStream;
use crate::queue::WorkQueue;
use crate::utils::{calc_request, read_piece};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, broadcast};
use tokio::net::TcpStream;
use tokio::prelude::*;
use std::time::Duration;
use tokio::time::timeout;
use std::error::Error;
use std::cmp::min;

pub struct Worker {
    id: u64,
    progress: Arc<Mutex<Progress>>,
    peers: WorkQueue<String>,
    work: WorkQueue<torrents::Piece>,
    handshake: Vec<u8>,
    info_hash: Vec<u8>,
    bf: Arc<Mutex<bitfield::Bitfield>>,

    stream: OpStream,
    current: Option<torrents::Piece>,
    requested: usize,
    received: usize,
    buf: Vec<u8>,
    rx: broadcast::Receiver<Message>,
    tx: mpsc::Sender<(u64, usize, Vec<u8>)>,
}

enum State {
    Choked,
    Unchoked
}

impl Worker {
    pub fn from_client(c: &client::Client, i: u64, rx: broadcast::Receiver<Message>, tx: mpsc::Sender<(u64, usize, Vec<u8>)>) -> Worker {
        Worker {
            id: i+1,
            progress: Arc::clone(&c.progress),
            peers: c.peer_list.clone(),
            work: c.torrent.pieces.clone(),
            handshake: c.handshake.clone(),
            info_hash: c.torrent.info_hash.clone(),
            bf: c.bf.clone(),

            stream: OpStream::new(),
            current: None,
            requested: 0,
            received: 0,
            buf: Vec::new(),
            rx,
            tx,
        }
    }

    // tries to connect to ip and handshake (with timeouts)
    async fn handshake(&self, ip: &str) -> Result<TcpStream, Box<dyn Error>> {
        let mut s = timeout(consts::TIMEOUT, TcpStream::connect(ip)).await??;

        let mut buf = [0; 68];
        timeout(consts::TIMEOUT, s.write_all(self.handshake.as_slice())).await??;
        if let Ok(n) = s.read(&mut buf).await {
            println!("READ {}", n);
        }

        if let Some(h) = Handshake::deserialize(buf.to_vec()) {
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
            loop {
                if let Some(s) = self.peers.pop().await {
                    ip = s;
                    break
                } else {
                    // wait 1s before getting another peer
                    tokio::time::delay_for(Duration::from_secs(1)).await;
                }
            }

            println!("Worker {} attempting to connect to {}", self.id, ip);
            let res = self.handshake(&ip).await.ok();

            if let Some(s) = res {
                self.stream = OpStream::from(s);

                // send own bitfield
                let payload: Vec<u8>;
                {
                    let b = self.bf.lock().await;
                    payload = b.bf.clone();
                }
                if let Err(_) = self.stream.send_message(messages::Message {
                    message_id: MessageID::Bitfield,
                    payload: Some(payload),
                }).await {
                    continue
                }

                // get opposing bitfield
                let response = self.stream.read_message().await;
                if let Some(msg) = response {
                    if msg.message_id == MessageID::Bitfield && msg.payload != None {
                        return bitfield::Bitfield {
                            bf: msg.payload.unwrap(),
                        }
                    }
                }
            }
            println!("Worker {} not connected", self.id);
            // put ip back
            self.peers.push(ip).await;
        }
    }

    // sends requests and pieces and chooses new work piece if necessary
    // returns false if error or no more work to be done
    async fn manage_io(&mut self, bf: &bitfield::Bitfield) -> Option<()> {
        if self.current == None {
            self.current = self.work.find_first(|x| bf.has(x.1)).await;
            self.requested = 0;
            self.received = 0;

            let piece = self.current.as_ref()?;
            self.buf = vec![0; piece.2];
        }

        let piece = self.current.as_ref()?;
        while self.requested < min(piece.2, self.received + consts::MAXREQUESTS * consts::BLOCKSIZE) {
            let (msg, len) = calc_request(piece.1, self.requested, piece.2)?;
            // println!("Worker {} requesting index {}, start {}", self.id, piece.1, self.requested);
            self.stream.send_message(Message {
                message_id: MessageID::Request,
                payload: Some(msg),
            }).await.ok()?;
            self.requested += len;
        }

        Some(())
    }

    async fn process_piece(&mut self, payload: Vec<u8>) -> Option<()> {
        let piece = self.current.clone()?;
        let (idx, start, buf) = read_piece(payload)?;
        // println!("Got chunk {}, {}", idx, start);
        if idx != piece.1 {
            println!("Not correct piece");
            return None
        }
        if start + buf.len() > self.buf.len() {
            println!("Too large payload!");
            println!("{} + {} > {}", start, buf.len(), self.buf.len());
            return None
        }

        self.buf[start..start+buf.len()].copy_from_slice(buf.as_slice());
        self.received += buf.len();

        if self.received == piece.2 {
            // received all of piece
            self.received = 0;
            self.current = None;

            // put piece back if doesn't match hash
            if piece.verify(&self.buf) {
                if self.tx.send((self.id, idx, self.buf.clone())).await.ok() == None {
                    println!("Couldn't send!");
                    return None
                }
            } else {
                self.work.push(piece).await;
                println!("Couldn't verify!");
                return None
            }
        }

        Some(())
    }

    pub async fn download(&mut self) {
        loop {
            let bf = self.connect().await;

            // get first piece
            self.current = self.work.find_first(|x| bf.has(x.1)).await;
            if let Some(piece) = self.current.as_ref() {
                let res = self.stream.send_message(Message{
                    message_id: MessageID::Interested,
                    payload: None,
                }).await.ok();
                self.buf = vec![0; piece.2];

                if res == None {
                    continue
                }
            } else {
                println!("Worker {}, no need pieces", self.id);
                continue
            }

            println!("Worker {} connected", self.id);
            self.received = 0;
            self.requested = 0;

            let mut state = State::Choked;

            loop {
                tokio::select! {
                    _ = self.rx.recv() => {
                        println!("Worker {} pinged!", self.id);
                    }
                    response = self.stream.read_message() => {
                        if let Some(msg) = response {
                            match state {
                                State::Choked => {
                                    match msg.message_id {
                                        MessageID::Unchoke => {
                                            println!("Worker {} unchoked!", self.id);
                                            state = State::Unchoked;
                                            if self.manage_io(&bf).await == None {
                                                println!("BAD");
                                                break
                                            }
                                        },
                                        MessageID::Piece => {
                                            if self.process_piece(msg.payload.unwrap()).await == None {
                                                break
                                            }
                                        },
                                        _ => ()
                                    }
                                },

                                State::Unchoked => {
                                    match msg.message_id {
                                        MessageID::Choke => {
                                            // println!("Worker {} choked!", self.id);
                                            state = State::Choked;
                                        },
                                        MessageID::Piece => {
                                            if self.process_piece(msg.payload.unwrap()).await == None {
                                                break
                                            }
                                            if self.manage_io(&bf).await == None {
                                                break
                                            }
                                        },
                                        _ => (),
                                    }
                                },
                            }
                        } else {
                            println!("Worker {} read error", self.id);
                            break
                        }
                    }
                }
            }
            println!("Worker {} breaking off", self.id);
            // if there is still work, return it to queue
            if let Some(piece) = self.current.take() {
                self.work.push(piece).await;
            }
        }
    }
}
