use crate::client::Progress;
use crate::torrents;
use crate::messages::handshake::Handshake;
use crate::messages::messages::{Message, MessageID};
use crate::messages::ops;
use crate::consts;
use crate::utils::bitfield;
use crate::client;
use crate::opstream::OpStream;
use crate::utils::queue::WorkQueue;
use crate::utils::{calc_request, read_piece};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, broadcast};
use tokio::net::TcpStream;
use tokio::prelude::*;
use std::time::Duration;
use tokio::time::timeout;
use std::cmp::min;
use byteorder::{ByteOrder, BigEndian};

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
    brx: broadcast::Receiver<ops::Op>,
    tx: mpsc::Sender<ops::Op>,
}

impl Worker {
    pub fn from_client(c: &client::Client, i: u64, brx: broadcast::Receiver<ops::Op>, tx: mpsc::Sender<ops::Op>) -> Worker {
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
            brx,
            tx,
        }
    }

    // tries to connect to ip and handshake (with timeouts)
    async fn handshake(&self, ip: &str) -> Option<TcpStream> {
        let mut s = timeout(consts::TIMEOUT, TcpStream::connect(ip)).await.ok()?.ok()?;

        let mut buf = [0; 68];
        timeout(consts::TIMEOUT, s.write_all(self.handshake.as_slice())).await.ok()?.ok()?;
        timeout(consts::TIMEOUT, s.read(&mut buf)).await.ok()?.ok()?;

        if let Some(h) = Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.info_hash {
                return None
            }
        }
        Some(s)
    }

    // repeatedly attempts to connect to a peer,
    // handshake and exchange bitfield until success
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
            let res = self.handshake(&ip).await;

            if let Some(s) = res {
                self.stream = OpStream::from(s);

                // send own bitfield
                let payload: Vec<u8>;
                {
                    let b = self.bf.lock().await;
                    payload = b.bf.clone();
                }
                if self.stream.send_message(Message {
                    message_id: MessageID::Bitfield,
                    payload: Some(payload),
                }).await == None {
                    continue
                }

                // get opposing bitfield
                let response = self.stream.read_message().await;
                if let Some(msg) = response {
                    if msg.message_id == MessageID::Bitfield && msg.payload != None {
                        let bf = msg.payload.unwrap();

                        {
                            // test length of bitfield
                            let own_bf = self.bf.lock().await;
                            if own_bf.bf.len() != bf.len() {
                                continue
                            }
                        }
                        return bitfield::Bitfield { bf }
                    }
                }
            }
            println!("Worker {} not connected", self.id);
            // put ip back
            self.peers.push(ip).await;
        }
    }

    // sends requests and pieces and chooses new work piece if necessary
    // returns true if requested, false if no work piece, and None if error
    async fn manage_io(&mut self, bf: &bitfield::Bitfield) -> Option<bool> {
        if self.current == None {
            self.current = self.work.find_first(|x| bf.has(x.1)).await;
            self.requested = 0;
            self.received = 0;

            let piece = match &self.current {
                Some(x) => x,
                None => return Some(false),
            };
            self.buf = vec![0; piece.2];
        }

        let piece = match &self.current {
            Some(x) => x,
            None => return Some(false)
        };
        while self.requested < min(piece.2, self.received + consts::MAXREQUESTS * consts::BLOCKSIZE) {
            let (msg, len) = calc_request(piece.1, self.requested, piece.2)?;
            self.stream.send_message(Message {
                message_id: MessageID::Request,
                payload: Some(msg),
            }).await?;
            self.requested += len;
        }

        Some(true)
    }

    // adds data from payload into buf
    async fn process_piece(&mut self, payload: Vec<u8>) -> Option<()> {
        let piece = self.current.clone()?;
        let (idx, start, buf) = read_piece(payload)?;

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
                if self.tx.send(ops::Op {
                    id: self.id,
                    op_type: ops::OpType::OpPiece(idx, self.buf.split_off(0)),
                }).await.ok() == None {
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

    // interacts with a live connection
    // assumes bitfields have been exchanges, but no current piece
    // if disconnect, then will disconnect if no pieces to work on
    async fn interact(&mut self, bf: bitfield::Bitfield, disconnect: bool) {
        // find first piece
        self.current = self.work.find_first(|x| bf.has(x.1)).await;
        if let Some(piece) = self.current.as_ref() {
            if self.stream.send_message(Message{
                message_id: MessageID::Interested,
                payload: None,
            }).await == None {
                return
            }

            self.buf = vec![0; piece.2];
        }

        if self.current == None && disconnect {
            return
        }

        println!("Worker {} connected", self.id);
        self.received = 0;
        self.requested = 0;

        let mut choked = true;

        loop {
            tokio::select! {
                op = self.brx.recv() => {
                    if let Err(_) = op {
                        break
                    }
                    let op = op.unwrap();

                    if op.id != 0 {
                        continue
                    }

                    match op.op_type {
                        ops::OpType::OpMessage(msg) => {
                            if self.stream.send_message(msg).await == None {
                                break
                            }
                        },
                        _ => ()
                    }
                },
                msg = self.stream.read_message() => {
                    if msg == None {
                        break
                    }

                    let msg = msg.unwrap();
                    match msg.message_id {
                        MessageID::Piece => {
                            if self.process_piece(msg.payload.unwrap()).await == None {
                                break
                            }
                        },
                        MessageID::Unchoke => {
                            println!("Worker {} unchoked!", self.id);
                            choked = false;
                        },
                        MessageID::Choke => {
                            choked = true;
                        },
                        MessageID::Have => {
                            let buf = msg.payload.unwrap();
                            let i = BigEndian::read_u32(&buf);
                            {
                                let mut self_bf = self.bf.lock().await;
                                self_bf.add(i as usize);
                            }
                        },
                        MessageID::Request => {
                            
                        },
                        MessageID::Interested => {
                            if self.stream.send_message(Message {
                                message_id: MessageID::Unchoke,
                                payload: None,
                            }).await == None {
                                break
                            }
                        },
                        _ => {
                        },
                    }

                    // if not choked, send some requests
                    if !choked {
                        match self.manage_io(&bf).await {
                            None => break,
                            Some(false) => {
                                if disconnect {
                                    break
                                }
                            },
                            _ => (),
                        }
                    }
                },
            }
        }

        println!("Worker {} disconnecting", self.id);
        // if there is still work, return it to queue
        if let Some(piece) = self.current.take() {
            self.work.push(piece).await;
        }
    }

    pub async fn download(&mut self) {
        loop {
            let bf = self.connect().await;
            self.interact(bf, true).await; 
            self.stream.close();
        }
    }
}
