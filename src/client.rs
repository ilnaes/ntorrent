use crate::torrents::Torrent;
use crate::messages;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use async_std::task;
use std::time::Duration;
use std::net::TcpStream;
use std::io::prelude::*;
use crate::utils::serialize_bytes;

pub struct Progress {
    pub uploaded: i64,
    pub downloaded: i64,
    pub left: i64,
}

pub struct Client {
    torrent: Torrent,
    handshake: Vec<u8>,
    interval: u64,
    peer_list: Vec<String>,
    progress: Progress,
    port: i64,
}

fn parse_peerlist(buf: Vec<u8>) -> Vec<String> {
    if buf.len() % 6 != 0 {
        panic!("Peer list not correct length!");
    }

    let n = buf.len() / 6;
    let mut cx = Cursor::new(buf);
    let mut res = Vec::new();

    for _ in 0..n {
        let mut s = String::new();
        for _ in 0..4 {
            s.push_str(&format!("{}.", cx.read_u8().unwrap()));
        }
        s.pop();
        s.push(':');
        s.push_str(&format!("{}", cx.read_u16::<BigEndian>().unwrap()));

        res.push(s);
    }
    res
}

impl Client { 
    pub fn new(s: &str) -> Client {
        let torrent = Torrent::new(s);
        let handshake = messages::Handshake::from(&torrent).serialize();

        Client {
            torrent,
            interval: 0,
            peer_list: Vec::new(),
            progress: Progress {
                uploaded: 0,
                downloaded: 0,
                left: 0,
            },
            port: 2222,
            handshake,
        }
    }
    pub async fn poll_peerlist(&mut self) {
        loop {
            self.get_peerlist().await;
            task::sleep(Duration::from_secs(self.interval)).await;
        }
    }


    pub async fn download(&mut self) {
        self.get_peerlist().await;

        let mut stream = TcpStream::connect(self.peer_list[0].clone()).expect("Could not connect to peer");

        let mut buf = [0; 128];

        stream.write(self.handshake.as_slice()).unwrap();
        stream.read(&mut buf).unwrap();

        if let Some(h) = messages::Handshake::deserialize(buf.to_vec()) {
            if h.info_hash != self.torrent.info_hash {
                println!("BAD!");
            } else {
                println!("GOOD!");
            }
        }
    }

    async fn get_peerlist(&mut self) {
        // manually encode bytes
        let url = format!("{}?info_hash={}&peer_id={}",
                          self.torrent.announce,
                          serialize_bytes(&self.torrent.info_hash),
                          serialize_bytes(&self.torrent.peer_id));
        let params = [("port", self.port),
                        ("uploaded", self.progress.uploaded),
                        ("downloaded", self.progress.downloaded),
                        ("compact", 1),
                        ("left", self.torrent.files
                                    .iter()
                                    .map(|x| x.length)
                                    .fold(0, |x,y| x+y))];

        let client = reqwest::Client::new();
        let res = client
            .get(url.as_str())
            .query(&params)
            .send()
            .await
            .expect("Could not parse tracker response!")
            .bytes()
            .await
            .expect("Could not parse tracker response!");

        let d: Result<messages::TrackerResponse, serde_bencode::error::Error> = serde_bencode::de::from_bytes(&res);
        if let Ok(f) = d {
            self.interval = f.interval;
            self.peer_list = parse_peerlist(f.peers.to_vec());
            println!("{:?}", self.peer_list);
        } else {
            panic!("Could not parse tracker response!");
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn peerlist_test() {
    }
}
