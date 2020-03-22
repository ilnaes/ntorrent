use crate::messages;
use crate::handshake::Handshake;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::queue::WorkQueue;
use crate::worker::Worker;
use crate::bitfield;
use tokio::sync::{Mutex, mpsc, broadcast};
use std::sync::Arc;
use byteorder::{BigEndian, WriteBytesExt};
use std::fs::File;
use std::io::prelude::*;

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
}

impl Client {
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        if torrent.length == 0 {
            panic!("no pieces");
        }
        let handshake = Handshake::from(&torrent).serialize();
        let n = (torrent.length - 1) / (8 * torrent.piece_length) + 1;
        let len = torrent.length;

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
                bf: vec![0; n],
            })),
        }
    }

    async fn manage_workers(&mut self) {
        let n = self.torrent.pieces.len().await;
        let (mtx, mrx) = mpsc::channel(n);
        let (btx, _) = broadcast::channel(n);
        
        for i in 0..self.nworkers {
            let tx = mtx.clone();
            let rx = btx.subscribe();
            let mut w = Worker::from_client(&self, i, rx, tx.clone());
            tokio::spawn(async move {
                w.download(true).await;
            });
        }

        // spin up receiver of pieces
        self.receive(mrx, btx).await;
    }

    async fn receive(&mut self, mut mrx: mpsc::Receiver<(u64, usize, Vec<u8>)>, btx: broadcast::Sender<messages::Message>) -> Option<()> {
        let n = self.torrent.pieces.len().await;
        let mut received: usize = 0;
        let mut buf = vec![0; self.torrent.length];

        while let Some((id, idx, res)) = mrx.recv().await {
            let start = idx * self.torrent.piece_length;
            buf[start..start + res.len()].copy_from_slice(res.as_slice());

            {
                // update progress
                let mut prog = self.progress.lock().await;
                prog.downloaded += res.len();
            }

            // broadcast HAVE to all workers
            let mut payload = vec![];
            WriteBytesExt::write_u32::<BigEndian>(&mut payload, idx as u32).ok()?;
            btx.send(messages::Message{
                message_id: messages::MessageID::Have,
                payload: Some(payload),
            }).ok();

            received += 1;
            println!("Client got piece {} from {} --- {:.3}%", idx, id, 100f32 * (received as f32)/(n as f32));
            
            if received == n {
                let mut f = File::open("what").ok()?;
                f.write(buf.as_slice()).ok()?;
                return Some(())
            }
        }
        Some(())
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        tokio::join!(peerlist.poll_peerlist(), self.manage_workers());
    }
}
