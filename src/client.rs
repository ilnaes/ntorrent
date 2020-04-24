use crate::messages::handshake::Handshake;
use crate::messages::messages::Message;
use crate::messages::ops::*;
use crate::partial::Partial;
use crate::peerlist::Peerlist;
use crate::torrents::Torrent;
use crate::utils::queue::Queue;
use crate::worker::Worker;
use ctrlc;
use std::sync::Arc;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc, Mutex};

pub struct Client<'a> {
    ndownloaders: u64,
    nlisteners: u64,
    pub torrent: &'a Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: Queue<String>,
    pub port: u16,
    pub btx: Arc<Mutex<broadcast::Sender<Op>>>,
    pub partial: Partial<'a>,
    channel_length: usize,
}

async fn listen(
    port: u16,
    peer_q: Queue<TcpStream>,
    mut done_q: mpsc::Receiver<()>,
    mut erx: broadcast::Receiver<()>,
) {
    let mut listener = match TcpListener::bind(format!("0.0.0.0:{}", port)).await {
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

impl<'a> Client<'a> {
    pub async fn from(torrent: &'a Torrent, port: u16) -> Client<'a> {
        if torrent.length == 0 {
            panic!("no pieces");
        }
        let handshake = Handshake::from(&torrent).serialize();

        let partial = Partial::from(torrent);

        let n = torrent.pieces.len().await;
        let (btx, _) = broadcast::channel(n);

        Client {
            port,
            partial,
            ndownloaders: 10,
            nlisteners: 10,
            torrent,
            peer_list: Queue::new(),
            handshake,
            btx: Arc::new(Mutex::new(btx)),
            channel_length: n,
        }
    }

    // spawns downloaders and listeners
    async fn manage_workers(
        &mut self,
        mtx: mpsc::Sender<Op>,
        peer_q: Queue<TcpStream>,
        mut done_q: mpsc::Sender<()>,
        download: bool,
    ) -> Vec<mpsc::Sender<Op>> {
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
                let (mtx1, mrx1) = mpsc::channel::<Op>(self.channel_length);
                vec_mtx.push(mtx1);
                let mut w = Worker::from_client(&self, i + 1, rx, mrx1, mtx.clone());
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
            let (mtx1, mrx1) = mpsc::channel::<Op>(self.channel_length);
            vec_mtx.push(mtx1);
            let pq = peer_q.clone();
            let dq = done_q.clone();

            let mut w = Worker::from_client(&self, i + 1, rx, mrx1, mtx.clone());
            tokio::spawn(async move {
                w.upload(pq, dq).await;
            });
            done_q.send(()).await.ok();
            i += 1;
        }

        vec_mtx
    }

    // receives pieces and signals have messages
    async fn receive(
        &mut self,
        mut mtx: Vec<mpsc::Sender<Op>>,
        mut mrx: mpsc::Receiver<Op>,
        mut erx: broadcast::Receiver<()>,
    ) {
        let mut received: usize = 0;
        let mut served = 0;

        loop {
            tokio::select! {
                Ok(()) = erx.recv() => {
                    // broadcast STOP to all workers
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
                            if let Some(b) = self.partial.get(i, s, len).await {
                                // send piece
                                mtx[op.id as usize-1].send(Op {
                                    id: 0,
                                    op_type: OpType::OpMessage(Message::Piece(i, s, b)),
                                }).await.ok();
                                served += len as usize;
                                println!("Serving piece {} to Worker {} --- {:.2} KB uploaded", i, op.id, served as f64/1024f64);
                            } else {
                                mtx[i as usize-1].send(Op {
                                    id: 0,
                                    op_type: OpType::OpDisconnect,
                                }).await.ok();
                            }
                        },
                        OpType::OpPiece(idx, res) => {
                            if let Some(finished) = self.partial.update(idx, res.clone()).await {
                                // broadcast HAVE to all workers
                                let btx = self.btx.lock().await;
                                btx.send(Op {
                                    id: 0,
                                    op_type: OpType::OpMessage(Message::Have(idx))
                                }).ok();

                                if finished {
                                    btx.send(Op {
                                        id: 0,
                                        op_type: OpType::OpDownStop,
                                    }).ok();
                                }
                            }

                            println!("Got piece {} from Worker {} --- {:.2}%", idx, op.id, 100f32 * (received as f32)/(self.channel_length as f32));
                            received += 1;
                        },
                        _ => (),
                    }
                }
            }
        }

        println!("Receiver stopping");
    }

    pub async fn serve(&mut self, download: bool) {
        let mut peerlist = Peerlist::from(&self);

        // channel for client <- workers
        let (mtx, mrx) = mpsc::channel(std::cmp::max(self.channel_length, 10));

        // done queue for listeners
        let (done_tx, done_rx) = mpsc::channel(self.nlisteners as usize);

        let peer_q = Queue::new();

        let (tx, rx) = broadcast::channel(1);
        let tx1 = tx.clone();

        // vector of channels for workers <- client
        let vec_mtx = self
            .manage_workers(mtx, peer_q.clone(), done_tx, download)
            .await;

        let port = self.port;

        ctrlc::set_handler(move || {
            if let Err(_) = tx1.send(()) {
                std::process::exit(1)
            }
        })
        .expect("Could not register SIGINT handler");

        tokio::join!(
            peerlist.poll_peerlist(tx.subscribe()),
            self.receive(vec_mtx, mrx, rx),
            listen(port, peer_q, done_rx, tx.subscribe())
        );
    }
}
