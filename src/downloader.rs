use crate::client::{Client, Progress};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use std::collections::VecDeque;

pub struct Manager {
    nworkers: u64,
    progress: Arc<Mutex<Progress>>,
    peer_list: Arc<Mutex<Vec<String>>>,
    info_hash: Vec<u8>,
    peer_id: Vec<u8>,
    pieces: Arc<Mutex<VecDeque<[u8; 20]>>>,
}

impl Manager {
    pub fn from(c: &Client, nworkers: u64) -> Manager {
        Manager {
            nworkers,
            progress: Arc::clone(&c.progress),
            peer_list: Arc::clone(&c.peer_list),
            info_hash: c.torrent.info_hash.clone(),
            peer_id: c.torrent.peer_id.clone(),
            pieces: Arc::new(Mutex::new(c.torrent.pieces.clone())),
        }
    }

    pub async fn download(&self) {
    //     let mut stream = TcpStream::connect(self.peer_list[0].clone()).expect("Could not connect to peer");

    //     let mut buf = [0; 128];

    //     stream.write(self.handshake.as_slice()).unwrap();
    //     stream.read(&mut buf).unwrap();

    //     if let Some(h) = messages::Handshake::deserialize(buf.to_vec()) {
    //         if h.info_hash != self.torrent.info_hash {
    //             println!("BAD!");
    //         } else {
    //             println!("GOOD!");
    //         }
    //     }
    }
}
