use crate::client::Progress;
use crate::torrents::Piece;
use crate::messages::handshake::Handshake;
use crate::messages::messages::Message;
use crate::messages::ops::*;
use crate::consts::*;
use crate::utils::bitfield::Bitfield;
use crate::client;
use crate::opstream::OpStream;
use crate::utils::queue::Queue;
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
    peers: Queue<String>,
    work: Queue<Piece>,
    handshake: Vec<u8>,
    info_hash: Vec<u8>,
    bf: Arc<Mutex<Bitfield>>,
    disconnect: bool,

    stream: OpStream,
    brx: broadcast::Receiver<Op>,  // broadcast receiver from client
    mrx: mpsc::Receiver<Op>,       // individual receiver from client
    tx: mpsc::Sender<Op>,          // transmitter back to client

    buf: Vec<u8>,
    current: Option<Piece>,
    requested: u32,
    received: u32,
    choked: bool,
}

impl Worker {
    pub fn from_client(c: &client::Client, i: u64, brx: broadcast::Receiver<Op>, mrx: mpsc::Receiver<Op>, tx: mpsc::Sender<Op>) -> Worker {
        Worker {
            id: i,
            progress: Arc::clone(&c.progress),
            peers: c.peer_list.clone(),
            work: c.torrent.pieces.clone(),
            handshake: c.handshake.clone(),
            info_hash: c.torrent.info_hash.clone(),
            bf: c.bf.clone(),
            disconnect: false,

            stream: OpStream::new(),
            brx,
            mrx,
            tx,

            current: None,
            requested: 0,
            received: 0,
            buf: Vec::new(),
            choked: true,
        }
    }

    // tries to connect to ip (with timeouts)
    async fn connect(&self, ip: &str) -> Option<TcpStream> {
        Some(timeout(TIMEOUT, TcpStream::connect(ip)).await.ok()?.ok()?)
    }

    // exchanges handshakes and exchanges bitfield with TcpStream
    // returns opposing bitfield if successful
    async fn protocol(&mut self, mut s: TcpStream) -> Option<Bitfield> {
        let mut buf = [0; 68];
        if self.disconnect {
            // inititor sends first handshake
            timeout(TIMEOUT, s.write_all(self.handshake.as_slice())).await.ok()?.ok()?;
        }
        timeout(TIMEOUT, s.read_exact(&mut buf)).await.ok()?.ok()?;

        if Handshake::deserialize(buf.as_ref())?.info_hash != self.info_hash {
            return None
        }
        if !self.disconnect {
            timeout(TIMEOUT, s.write_all(self.handshake.as_slice())).await.ok()?.ok()?;
        }

        self.stream = OpStream::from(s);

        // send own bitfield
        let payload: Vec<u8>;
        {
            let b = self.bf.lock().await;
            payload = b.bf.clone();
        }
        if self.stream.send_message(Message::Bitfield(payload)).await == None {
            return None
        }

        // get opposing bitfield
        let response = self.stream.read_message().await;
        let msg = response?;
        if let Message::Bitfield(bf) = msg {
            {
                // test length of bitfield
                let own_bf = self.bf.lock().await;
                if own_bf.bf.len() != bf.len() {
                    return None
                }
            }
            Some(Bitfield { bf })
        } else {
            None
        }
    }

    // repeatedly attempts to connect to a peer,
    // and handshake until success
    async fn reach_peer(&mut self) -> TcpStream {
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

            if let Some(s) = self.connect(&ip).await {
                return s
            }

            // put ip back
            self.peers.push(ip).await;
        }
    }

    async fn get_piece(&mut self, bf: &Bitfield) -> Option<()> {
        if self.current == None {
            self.current = self.work.find_first(|x| bf.has(x.1 as usize)).await;
            self.requested = 0;
            self.received = 0;

            let piece = self.current.as_ref()?;
            self.buf = vec![0; piece.2 as usize];
        }

        Some(())
    }

    // sends requests and pieces and chooses new work piece if necessary
    // returns true if requested and None if error
    async fn manage_io(&mut self) -> Option<()> {
        let piece = self.current.as_ref()?;
        while self.requested < min(piece.2, self.received + MAXREQUESTS * BLOCKSIZE) {
            let len = calc_request(self.requested, piece.2);
            self.stream.send_message(Message::Request(
                                        piece.1,
                                        self.requested,
                                        len
                                    )).await?;
            self.requested += len;
        }

        Some(())
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
                if self.tx.send(Op {
                    id: self.id,
                    op_type: OpType::OpPiece(i, self.buf.clone()),
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
    async fn process_msg(&mut self, msg: Option<Message>, bf: &mut Bitfield) -> Option<()> {
        match msg? {
            Message::Piece(idx, start, payload) => {
                self.process_piece(idx, start, payload).await?;
            },
            Message::Unchoke => {
                self.choked = false;
            },
            Message::Choke => {
                self.choked = true;
            },
            Message::Have(i) => {
                bf.add(i as usize);
            },
            Message::Interested => {
                self.stream.send_message(Message::Unchoke).await?;
                self.disconnect = false
            },
            Message::NotInterested => {
                if self.current == None {
                    return None
                }
                self.disconnect = true
            },
            Message::Request(i, s, len) => {
                self.tx.send(Op {
                    id: self.id,
                    op_type: OpType::OpRequest(i, s, len),
                }).await.ok()?;
            },
            _ => {
            },
        }

        Some(())
    }

    // processes operations from the client receiver
    // returns Some(()) if successful, otherwise None
    async fn process_op(&mut self, op: Option<Op>) -> Option<()> {
        let op = op?;

        if op.id != 0 {
            // not receiver id
            return None
        }

        match op.op_type {
            OpType::OpMessage(msg) => {
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
            OpType::OpDisconnect => {
                None
            }
            _ => None
        }
    }

    // interacts with a live connection
    // assumes bitfields have been exchanges, but no current piece
    async fn interact(&mut self, mut bf: Bitfield) {
        self.choked = true;

        // find first piece
        if self.get_piece(&bf).await == None && self.disconnect {
            return
        }
        if self.stream.send_message(Message::Interested).await == None {
            return
        }
        println!("Worker {} connected", self.id);

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
                    if self.process_msg(msg, &mut bf).await == None {
                        break
                    }
                },
            }

            if self.get_piece(&bf).await == None && self.disconnect {
                return
            }
            // if not choked, send some requests
            if !self.choked && self.manage_io().await == None {
                break
            }
        }

        println!("Worker {} disconnecting", self.id);
        self.stream.close();
        // if there is still work, return it to queue
        if let Some(piece) = self.current.take() {
            self.work.push(piece).await;
        }
    }

    pub async fn upload(&mut self, mut peer_q: Queue<TcpStream>, mut done: mpsc::Sender<()>) {
        loop {
            self.disconnect = false;
            let peer = peer_q.pop_block().await;
            if let Ok(addr) = peer.peer_addr() {
                println!("Worker {} getting connection from {:?}", self.id, addr);
            }
            if let Some(bf) = self.protocol(peer).await {
                self.interact(bf).await;
                done.send(()).await.ok();
            }
        }
    }

    pub async fn download(&mut self) {
        loop {
            self.disconnect = true;
            let peer = self.reach_peer().await;
            if let Ok(addr) = peer.peer_addr() {
                println!("Worker {} attempting to connect to {:?}", self.id, addr);
            }
            if let Some(bf) = self.protocol(peer).await {
                self.interact(bf).await; 
            }
        }
    }
}
