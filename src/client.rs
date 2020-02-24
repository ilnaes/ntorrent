use crate::messages;
use crate::torrents::Torrent;
use crate::peerlist::Peerlist;
use crate::downloader::Manager;
use tokio::sync::Mutex;
// use byteorder::{BigEndian, ReadBytesExt};
// use std::io::prelude::*;
// use std::io::Cursor;
// use std::net::TcpStream;
use std::sync::Arc;
use std::collections::VecDeque;

pub struct Progress {
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
}

pub struct Client {
    pub torrent: Torrent,
    pub handshake: Vec<u8>,
    pub peer_list: Arc<Mutex<VecDeque<String>>>,
    pub progress: Arc<Mutex<Progress>>,
    pub port: i64,
}

impl Client {
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        let handshake = messages::Handshake::from(&torrent).serialize();

        Client {
            torrent,
            peer_list: Arc::new(Mutex::new(VecDeque::new())),
            progress: Arc::new(Mutex::new(Progress {
                uploaded: 0,
                downloaded: 0,
                // TODO: figure out left
                left: 0,
            })),
            port: 2222,
            handshake,
        }
    }

    pub async fn download(&mut self) {
        let mut peerlist = Peerlist::from(&self);
        let manager = Manager::from(&self, 4);
        tokio::join!(peerlist.poll_peerlist(), manager.download());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn peerlist_test() {}
}
