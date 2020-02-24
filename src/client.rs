use crate::messages;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use tokio::sync::Mutex;
// use byteorder::{BigEndian, ReadBytesExt};
// use std::io::prelude::*;
// use std::io::Cursor;
// use std::net::TcpStream;
use std::sync::Arc;

pub struct Progress {
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
}

pub struct Client {
    pub torrent: Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: Arc<Mutex<Vec<String>>>,
    pub progress: Arc<Mutex<Progress>>,
    pub port: i64,
}

impl Client {
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        let handshake = messages::Handshake::from(&torrent).serialize();

        Client {
            torrent,
            peer_list: Arc::new(Mutex::new(Vec::new())),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                left: 0,
            })),
            port: 2222,
            handshake,
        }
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        tokio::join!(peerlist.poll_peerlist());
    //     self.get_peerlist().await;

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

#[cfg(test)]
mod tests {
    #[test]
    fn peerlist_test() {}
}
