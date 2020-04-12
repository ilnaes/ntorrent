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
use ctrlc;

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
    buf: Vec<u8>,
}

async fn listen(port: u16, mut peer_q: Queue<TcpStream>, mut done_q: mpsc::Receiver<()>, mut erx: broadcast::Receiver<()>) {
    let mut listener = match TcpListener::bind(format!("127.0.0.1:{}", port)).await {
        Ok(l) => l,
        Err(e) => panic!("Can't bind to port: {}", e),
    };
    println!("Listening on port {}", port);

    while let Some(()) = done_q.recv().await {
        tokio::select! {
            Ok((socket, _addr)) = listener.accept() => {
                peer_q.push(socket).await;
            },
            Ok(()) = erx.recv() => {
                break
            }
        }
    }

    println!("Listener stopping");
}

impl Client {
    pub async fn new(s: &str, dir: String, port: u16) -> Client {
        let torrent = Torrent::new(s, dir.clone());
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
            buf: vec![0; len],
        }
    }

    // returns None if no files exist
    // Some(true) if all files exist and are verified
    // Some(false) otherwise
    pub async fn has(&mut self) -> Option<bool> {
        let mut all = true;
        let mut exists = false;
        let mut i = 0;
        let mut buf = vec![0; self.torrent.length];

        for f in self.torrent.files.iter() {
            let path = f.path.join("/");
            let path = Path::new(path.as_str());

            // see if path exist
            if path.exists() {
                exists = true;
                let file = std::fs::read(path);
                if let Ok(v) = file {
                    if v.len() == f.length {
                        buf[i..i+f.length as usize].copy_from_slice(v.as_slice());
                    } else {
                        all = false;
                    }
                } else {
                    all = false;
                }
            } else {
                all = false;
            }
            i += f.length;
        }

        if !exists {
            None
        } else {
            if all {
                // verify all pieces
                {
                    let q = self.torrent.pieces.get_q();
                    let q = q.lock().await;
                    let mut iter = q.iter();
                    let mut i: u64 = 0;
                    let len = self.torrent.piece_length as usize;

                    while let Some(piece) = iter.next() {
                        let ceil = std::cmp::min(i as usize+len, self.torrent.length);
                        if !piece.verify(&buf[i as usize..ceil]) {
                            all = false;
                            break
                        } else {
                            if (i/(len as u64)) % 100 == 99 {
                                println!("Verified {:.2}%", 100f64 * (i+len as u64) as f64/self.torrent.length as f64);
                            }
                        }
                        i += len as u64;
                    }
                }

                if all {
                    self.buf = buf;
                    let mut p = self.progress.lock().await;
                    p.left = 0;

                    let mut bf = self.bf.lock().await;
                    let len = bf.len();
                    *bf = Bitfield::from(vec![255; len]);

                    self.torrent.pieces.clear().await;
                }
            }
            Some(all)
        }
    }


    // spawns downloaders and listeners
    async fn manage_workers(&mut self, mtx: mpsc::Sender<Op>, peer_q: Queue<TcpStream>, mut done_q: mpsc::Sender<()>, download: bool) -> Vec<mpsc::Sender<Op>> {
        let mut vec_mtx = Vec::new();
        let mut i = 0;

        if download {
            // spawn downloaders
            for _ in 0..self.ndownloaders {
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
                i += 1;
            }
        }

        // spawn listeners
        for _ in 0..self.nlisteners {
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
            i += 1;
        }

        vec_mtx
    }

    // receives pieces and signals have messages
    async fn receive(&mut self, mut mtx: Vec<mpsc::Sender<Op>>, mut mrx: mpsc::Receiver<Op>, mut erx: broadcast::Receiver<()>) {
        let n = self.torrent.pieces.len().await;
        let mut received: usize = 0;
        let mut served = 0;

        let path = if self.dir.len() > 0 {
            format!("{}/{}.part", self.dir, self.torrent.name)
        } else {
            format!("{}.part", self.torrent.name)
        };
        let mut partial =
            if let Ok(f) = File::create(path) {
                Some(f)
            } else {
                return
            };

        loop {
            tokio::select! {
                Ok(()) = erx.recv() => {
                    // broadcast HAVE to all workers
                    let btx = self.btx.lock().await;
                    btx.send(Op {
                        id: 0,
                        op_type: OpType::OpStop
                    }).ok();

                    break
                },
                Some(op) = mrx.recv() => {
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
                                    i, s, self.buf[start+s as usize..start+s as usize+len as usize].to_vec(),
                                )),
                            }).await.ok();
                            served += len as usize;
                            println!("Serving piece {} to Worker {} --- {:.2} KB uploaded", i, op.id, served as f64/1024f64);
                        },
                        OpType::OpPiece(idx, res) => {
                            let start = idx as usize * self.torrent.piece_length as usize;
                            self.buf[start..start + res.len()].copy_from_slice(res.as_slice());
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

                            if let Some(mut f) = partial.as_ref() {
                                if let Err(_) = f.write_all(res.as_slice()) {
                                    panic!("Can't write");
                                }
                            } else {
                                panic!("Can't write")
                            }

                            received += 1;
                            
                            println!("Got piece {} from Worker {} --- {:.2}%", idx, op.id, 100f32 * (received as f32)/(n as f32));
                            if received == n {
                                self.write_file(self.buf.as_slice());
                                let btx = self.btx.lock().await;
                                btx.send(Op {
                                    id: 0,
                                    op_type: OpType::OpStop,
                                }).ok();
                                partial = None;
                            }
                        },
                        _ => (),
                    }
                }
            }
        }

        println!("Receiver stopping");
    }

    fn write_file(&self, buf: &[u8]) {
        let mut i = 0;

        for f in self.torrent.files.iter() {
            let path = &f.path.join("/");
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

    pub async fn serve(&mut self, download: bool) {
        let mut peerlist = Peerlist::from(&self);
        let n = self.torrent.pieces.len().await;

        // channel for client <- workers
        let (mtx, mrx) = mpsc::channel(std::cmp::max(n, 10));

        // done queue for listeners
        let (done_tx, done_rx) = mpsc::channel(self.nlisteners as usize);

        let peer_q = Queue::new();

        let (tx, rx) = broadcast::channel(1);
        let tx1 = tx.clone();

        // vector of channels for workers <- client
        let vec_mtx = self.manage_workers(mtx, peer_q.clone(), done_tx, download).await;

        let port = self.port;

        ctrlc::set_handler(move || {
            if let Err(_) = tx1.send(()) {
                std::process::exit(1)
            }
        }).expect("Could not register SIGINT handler");

        tokio::join!(
            peerlist.poll_peerlist(tx.subscribe()),
            self.receive(vec_mtx, mrx, rx),
            listen(port, peer_q, done_rx, tx.subscribe())
        );
    }
}
