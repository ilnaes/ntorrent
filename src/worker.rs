use crate::client::Progress;
use crate::torrents;
use crate::messages::handshake::Handshake;
use crate::messages::messages::Message;
use crate::messages::ops;
use crate::consts;
use crate::utils::bitfield;
use crate::client;
use crate::opstream::OpStream;
use crate::utils::queue::WorkQueue;
use crate::utils::calc_request;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex, broadcast};
use tokio::net::TcpStream;
use tokio::prelude::*;
use std::time::Duration;
use tokio::time::timeout;
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
    requested: u32,
    received: u32,
    buf: Vec<u8>,
    brx: broadcast::Receiver<ops::Op>,
    mrx: mpsc::Receiver<ops::Op>,
    tx: mpsc::Sender<ops::Op>,
    choked: bool,
    disconnect: bool,
}

impl Worker {
    pub fn from_client(c: &client::Client, i: u64, brx: broadcast::Receiver<ops::Op>, mrx: mpsc::Receiver<ops::Op>, tx: mpsc::Sender<ops::Op>, disconnect: bool) -> Worker {
        Worker {
            id: i,
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
            mrx,
            tx,
            choked: true,
            disconnect,
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
                if self.stream.send_message(Message::Bitfield(payload)).await == None {
                    continue
                }

                // get opposing bitfield
                let response = self.stream.read_message().await;
                if let Some(msg) = response {
                    if let Message::Bitfield(bf) = msg {
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
            self.current = self.work.find_first(|x| bf.has(x.1 as usize)).await;
            self.requested = 0;
            self.received = 0;

            let piece = match &self.current {
                Some(x) => x,
                None => return Some(false),
            };
            self.buf = vec![0; piece.2 as usize];
        }

        let piece = match &self.current {
            Some(x) => x,
            None => return Some(false)
        };
        while self.requested < min(piece.2,
                        self.received + consts::MAXREQUESTS * consts::BLOCKSIZE) {
            let len = calc_request(self.requested, piece.2);
            self.stream.send_message(Message::Request(
                                        piece.1,
                                        self.requested,
                                        len
                                    )).await?;
            self.requested += len;
        }

        Some(true)
    }

    // adds data into buf
    async fn process_piece(&mut self, i: u32, s: u32, buf: Vec<u8>) -> Option<()> {
        let piece = self.current.clone()?;

        if i != piece.1 {
            println!("Not correct piece");
            return None
        }
        if s as usize + buf.len() > self.buf.len() {
            println!("Too large payload!");
            println!("{} + {} > {}", s, buf.len(), self.buf.len());
            return None
        }

        self.buf[s as usize..s as usize+buf.len()].copy_from_slice(buf.as_slice());
        self.received += buf.len() as u32;

        if self.received >= piece.2 {
            // received all of piece
            self.received = 0;
            self.current = None;

            // put piece back if doesn't match hash
            if piece.verify(&self.buf) {
                if self.tx.send(ops::Op {
                    id: self.id,
                    op_type: ops::OpType::OpPiece(i, self.buf.clone()),
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

    // processes messages from other clients
    async fn process_msg(&mut self, msg: Option<Message>, bf: &bitfield::Bitfield) -> Option<()> {
        match msg.unwrap() {
            Message::Piece(idx, start, payload) => {
                if self.process_piece(idx, start, payload).await == None {
                    return None
                }
            },
            Message::Unchoke => {
                println!("Worker {} unchoked!", self.id);
                self.choked = false;
            },
            Message::Choke => {
                self.choked = true;
            },
            Message::Have(i) => {
                {
                    let mut self_bf = self.bf.lock().await;
                    self_bf.add(i as usize);
                }
            },
            Message::Interested => {
                self.stream.send_message(Message::Unchoke).await?;
            },
            Message::Request(i, s, len) => {
                self.tx.send(ops::Op {
                    id: self.id,
                    op_type: ops::OpType::OpRequest(i, s, len),
                }).await.ok()?;
            },
            _ => {
            },
        }

        // if not choked, send some requests
        if !self.choked {
            if !self.manage_io(bf).await? && self.disconnect {
                return None
            }
        }

        Some(())
    }

    // processes operations from the client receiver
    // returns Some(()) if successful, otherwise None
    async fn process_op(&mut self, op: Option<ops::Op>) -> Option<()> {
        let op = op.unwrap();

        if op.id != 0 {
            // not receiver id
            return None
        }

        match op.op_type {
            ops::OpType::OpMessage(msg) => {
                // send message and update length sent
                let mut n = 0;
                if let Message::Piece(_,_,v) = &msg {
                    n += v.len();
                }
                if self.stream.send_message(msg).await == None {
                    return None
                }

                if n != 0 {
                    let mut prog = self.progress.lock().await;
                    prog.uploaded += n;
                }
                Some(())
            },
            ops::OpType::OpDisconnect => {
                None
            }
            _ => None
        }
    }

    // interacts with a live connection
    // assumes bitfields have been exchanges, but no current piece
    async fn interact(&mut self, bf: bitfield::Bitfield) {
        // find first piece
        self.current = self.work.find_first(|x| bf.has(x.1 as usize)).await;
        if let Some(piece) = &self.current {
            if self.stream.send_message(Message::Interested).await == None {
                return
            }

            self.buf = vec![0; piece.2 as usize];
        }

        if self.current == None && self.disconnect {
            return
        }

        println!("Worker {} connected", self.id);
        self.received = 0;
        self.requested = 0;
        self.choked = true;

        loop {
            tokio::select! {
                op = self.mrx.recv() => {
                    if self.process_op(op).await == None {
                        break
                    }
                },
                op = self.brx.recv() => {
                    if self.process_op(op.ok()).await == None {
                        break
                    }
                },
                msg = self.stream.read_message() => {
                    if self.process_msg(msg, &bf).await == None {
                        break
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
            self.interact(bf).await; 
            self.stream.close();
        }
    }
}
