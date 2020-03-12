use crate::messages;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::queue::WorkQueue;
use crate::worker::Worker;
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
    pub buf: Vec<u8>,
}

impl Client {
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        let left = torrent.files.iter().map(|x| x.length).fold(0, |a,b| a+b);
        let handshake = messages::Handshake::from(&torrent).serialize();

        Client {
            nworkers: 1,
            torrent,
            peer_list: WorkQueue::new(),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                left
            })),
            port: 2222,
            handshake,
            buf: vec![0; left],
        }
    }

    async fn manage_workers(&mut self) -> Option<()> {
        let n = self.torrent.pieces.len().await;
        let (mtx, mut mrx) = mpsc::channel(n);
        let (btx, _) = broadcast::channel(n);
        
        for i in 0..self.nworkers {
            let mut w = Worker::from_client(&self, i);
            let rx = btx.subscribe();
            let tx = mtx.clone();
            tokio::spawn(async move {
                w.download(tx, rx).await;
            });
        }

        let mut received = 0;

        while let Some((id, idx, res)) = mrx.recv().await {
            let start = idx * self.torrent.piece_length;
            self.buf[start..start + res.len()].copy_from_slice(res.as_slice());

            {
                // update progress
                let mut p = self.progress.lock().await;
                p.downloaded += res.len();
            }

            // broadcast HAVE to all workers
            let mut payload = vec![];
            WriteBytesExt::write_u32::<BigEndian>(&mut payload, idx as u32).unwrap(); 
            btx.send(messages::Message{
                message_id: messages::MessageID::Have,
                payload: Some(payload),
            }).ok();

            received += 1;
            println!("Client got piece {} from {} --- {:.3}%", idx, id, 100f32 * (received as f32)/(n as f32));
            
            if received == n {
                let mut f = File::open("what").ok()?;
                f.write(self.buf.as_slice()).ok()?;
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
