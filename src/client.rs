use crate::messages::messages::Message;
use crate::messages::handshake::Handshake;
use crate::messages::ops::*;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::utils::queue::Queue;
use crate::worker::Worker;
use crate::utils::bitfield::Bitfield;
use tokio::sync::{Mutex, mpsc, broadcast};
use tokio::net::{TcpStream, TcpListener};
use std::sync::Arc;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

pub struct Progress {
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
}

pub struct Client {
    ndownloaders: u64,
    nlisteners: u64,
    dir: String,
    pub torrent: Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: Queue<String>,
    pub progress: Arc<Mutex<Progress>>,
    pub port: u16,
    pub bf: Arc<Mutex<Bitfield>>,
    pub btx: Arc<Mutex<broadcast::Sender<Op>>>,
}

async fn listen(port: u16, mut peer_q: Queue<TcpStream>, mut done_q: mpsc::Receiver<()>) {
    let mut listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(l) => l,
        Err(e) => panic!("Can't bind to port: {}", e),
    };
    println!("Listening on port {}", port);

    while let Some(()) = done_q.recv().await {
        if let Ok((socket, _addr)) = listener.accept().await {
            peer_q.push(socket).await;
        }
    }
}

impl Client {
    pub async fn new(s: &str, dir: String, port: u16) -> Client {
        let torrent = Torrent::new(s);
        if torrent.length == 0 {
            panic!("no pieces");
        }
        let handshake = Handshake::from(&torrent).serialize();

        let bf_len = (torrent.length - 1) / (8 * torrent.piece_length as usize) + 1;
        let len = torrent.length;
        let n = torrent.pieces.len().await;
        let (btx, _) = broadcast::channel(n);

        Client {
            port,
            dir,
            ndownloaders: 10,
            nlisteners: 10,
            torrent,
            peer_list: Queue::new(),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                left: len,
            })),
            handshake,
            bf: Arc::new(Mutex::new(Bitfield {
                bf: vec![0; bf_len],
            })),
            btx: Arc::new(Mutex::new(btx)),
        }
    }

    // spawns downloaders and listeners
    async fn manage_workers(&mut self, mtx: mpsc::Sender<Op>, peer_q: Queue<TcpStream>, mut done_q: mpsc::Sender<()>) -> Vec<mpsc::Sender<Op>> {
        let mut vec_mtx = Vec::new();

        // spawn downloaders
        for i in 0..self.ndownloaders {
            let rx;
            {
                let btx = self.btx.lock().await;
                rx = btx.subscribe();
            }
            let (mtx1, mrx1) = mpsc::channel::<Op>(self.torrent.length);
            vec_mtx.push(mtx1);
            let mut w = Worker::from_client(&self, i+1, rx, mrx1, mtx.clone());
            tokio::spawn(async move {
                w.download().await;
            });
        }

        // spawn listeners
        for i in self.ndownloaders..self.ndownloaders+self.nlisteners {
            let rx;
            {
                let btx = self.btx.lock().await;
                rx = btx.subscribe();
            }
            let (mtx1, mrx1) = mpsc::channel::<Op>(self.torrent.length);
            vec_mtx.push(mtx1);
            let pq = peer_q.clone();
            let dq = done_q.clone();

            let mut w = Worker::from_client(&self, i+1, rx, mrx1, mtx.clone());
            tokio::spawn(async move {
                w.upload(pq, dq).await;
            });
            done_q.send(()).await.ok();
        }

        vec_mtx
    }

    // receives pieces and signals have messages
    async fn receive(&self, mut mtx: Vec<mpsc::Sender<Op>>, mut mrx: mpsc::Receiver<Op>) {
        let n = self.torrent.pieces.len().await;
        let mut received: usize = 0;
        let mut buf = vec![0; self.torrent.length];
        let mut served = 0;

        while let Some(op) = mrx.recv().await {
            match op.op_type {
                OpType::OpRequest(i, s, len) => {
                    {
                        let bf = self.bf.lock().await;
                        if !bf.has(i as usize) {
                            mtx[i as usize-1].send(Op {
                                id: 0,
                                op_type: OpType::OpDisconnect,
                            }).await.ok();
                            continue
                        }
                    }

                    // send piece
                    let start = i as usize * self.torrent.piece_length as usize;
                    mtx[op.id as usize-1].send(Op {
                        id: 0,
                        op_type: OpType::OpMessage(Message::Piece(
                            i, s, buf[start+s as usize..start+s as usize+len as usize].to_vec(),
                        )),
                    }).await.ok();
                    served += len as usize;
                    println!("Serving piece {} to Worker {} --- {:.2} KB uploaded", i, op.id, served as f64/1024f64);
                },
                OpType::OpPiece(idx, res) => {
                    let start = idx as usize * self.torrent.piece_length as usize;
                    buf[start..start + res.len()].copy_from_slice(res.as_slice());
                    {
                        // check if already has piece
                        let mut bf = self.bf.lock().await;
                        if bf.has(idx as usize) {
                            continue
                        }
                        bf.add(idx as usize);
                    }
                    {
                        // update progress
                        let mut prog = self.progress.lock().await;
                        prog.downloaded += res.len();
                        prog.left -= res.len();
                    }
                    {
                        // broadcast HAVE to all workers
                        let btx = self.btx.lock().await;
                        btx.send(Op {
                            id: 0,
                            op_type: OpType::OpMessage(Message::Have(idx))
                        }).ok();
                    }

                    received += 1;
                    
                    println!("Got piece {} from Worker {} --- {:.2}%", idx, op.id, 100f32 * (received as f32)/(n as f32));
                    if received == n {
                        self.write_file(buf.as_slice());
                        let btx = self.btx.lock().await;
                        btx.send(Op {
                            id: 0,
                            op_type: OpType::OpStop,
                        }).ok();
                    }
                },
                _ => (),
            }
        }
    }

    fn write_file(&self, buf: &[u8]) {
        let mut i = 0;

        for f in self.torrent.files.iter() {
            let mut path = self.dir.clone();
            if path.len() > 0 {
                path.push('/');
            }
            path.push_str(&f.path.join("/"));
            println!("Writing to {}", path);
            let path = Path::new(&path);

            // create directories if necessary
            let prefix = path.parent().unwrap();
            std::fs::create_dir_all(prefix).expect("Could not create directories");

            let mut file = File::create(&path)
                        .expect(&format!("Could not create {}", path.to_str().unwrap()));

            file.write(&buf[i..i+f.length]).expect("Could not write to file");
            i += f.length;
        }
        println!("FINISHED");
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        let n = self.torrent.pieces.len().await;

        // channel for client <- workers
        let (mtx, mrx) = mpsc::channel(n);

        // done queue for listeners
        let (done_tx, done_rx) = mpsc::channel(self.nlisteners as usize);

        let peer_q = Queue::new();

        // vector of channels for workers <- client
        let vec_mtx = self.manage_workers(mtx, peer_q.clone(), done_tx).await;

        tokio::join!(
            peerlist.poll_peerlist(),
            self.receive(vec_mtx, mrx),
            listen(self.port, peer_q, done_rx)
        );
    }
}
