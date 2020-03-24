use crate::messages::messages::Message;
use crate::messages::handshake::Handshake;
use crate::messages::ops::{Op, OpType};
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::utils::queue::WorkQueue;
use crate::worker::Worker;
use crate::utils::bitfield;
use tokio::sync::{Mutex, mpsc, broadcast};
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
    nworkers: u64,
    pub torrent: Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: WorkQueue<String>,
    pub progress: Arc<Mutex<Progress>>,
    pub port: i64,
    pub bf: Arc<Mutex<bitfield::Bitfield>>,
    pub btx: Arc<Mutex<broadcast::Sender<Op>>>,
}

impl Client {
    pub async fn new(s: &str) -> Client {
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
            nworkers: 1,
            torrent,
            peer_list: WorkQueue::new(),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                left: len,
            })),
            port: 2222,
            handshake,
            bf: Arc::new(Mutex::new(bitfield::Bitfield {
                bf: vec![0; bf_len],
            })),
            btx: Arc::new(Mutex::new(btx)),
        }
    }

    async fn manage_listeners(&self) {
    }

    // spawns downloaders
    async fn manage_workers(&self, mtx: mpsc::Sender<Op>) {
        for i in 0..self.nworkers {
            let tx = mtx.clone();
            let rx;
            {
                let btx = self.btx.lock().await;
                rx = btx.subscribe();
            }
            let mut w = Worker::from_client(&self, i, rx, tx.clone());
            tokio::spawn(async move {
                w.download().await;
            });
        }
    }

    // receives pieces and signals have messages
    async fn receive(&self, mut mrx: mpsc::Receiver<Op>) -> Option<()> {
        let n = self.torrent.pieces.len().await;
        let mut received: usize = 0;
        let mut buf = vec![0; self.torrent.length];

        while let Some(op) = mrx.recv().await {
            match op.op_type {
                OpType::OpRequest(i, s, len) => {
                    let mut have = false;
                    {
                        let bf = self.bf.lock().await;
                        if bf.has(i as usize) {
                            have = true
                        }
                    }

                    if !have {
                        
                    }
                },
                OpType::OpPiece(idx, res) => {
                    let start = idx as usize * self.torrent.piece_length as usize;
                    buf[start..start + res.len()].copy_from_slice(res.as_slice());

                    {
                        // update progress
                        let mut prog = self.progress.lock().await;
                        prog.downloaded += res.len();
                    }
                    // broadcast HAVE to all workers
                    {
                        let btx = self.btx.lock().await;
                        btx.send(Op {
                            id: 0,
                            op_type: OpType::OpMessage(Message::Have(idx))
                        }).ok()?;
                    }

                    received += 1;
                    println!("Got piece {} from Worker {} --- {:.2}%", idx, op.id, 100f32 * (received as f32)/(n as f32));
                    
                    if received == n {
                        return self.write_file(buf.as_slice());
                    }
                },
                _ => (),
            }
        }
        Some(())
    }

    fn write_file(&self, buf: &[u8]) -> Option<()> {
        let path = Path::new("what.pdf");
        let mut f = File::create(&path).ok()?;
        f.write(buf).ok()?;
        println!("FINISHED");
        Some(())
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        let n = self.torrent.pieces.len().await;
        let (mtx, mrx) = mpsc::channel(n);

        tokio::join!(
            peerlist.poll_peerlist(),
            self.manage_workers(mtx.clone()),
            self.manage_listeners(),
            self.receive(mrx),
        );
    }
}
