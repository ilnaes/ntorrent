use crate::client::Progress;
use crate::utils::serialize_bytes;
use crate::client::Client;
use crate::utils::queue::Queue;
use tokio::sync::Mutex;
use std::sync::Arc;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use std::time::Duration;
use std::collections::VecDeque;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Serialize, Deserialize, Debug)]
struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

pub struct Peerlist {
    progress: Arc<Mutex<Progress>>,
    interval: u64,
    announce: String,
    info_hash: Vec<u8>,
    peer_id: Vec<u8>,
    port: u16,
    list: Queue<String>,
}

fn parse_peerlist(buf: &[u8]) -> VecDeque<String> {
    if buf.len() % 6 != 0 {
        panic!("Peer list not correct length!");
    }

    let n = buf.len() / 6;
    let mut cx = Cursor::new(buf);
    let mut res = VecDeque::new();

    for _ in 0..n {
        let mut s = String::new();
        for _ in 0..4 {
            s.push_str(&format!("{}.", cx.read_u8().unwrap()));
        }
        s.pop();
        s.push(':');
        s.push_str(&format!("{}", cx.read_u16::<BigEndian>().unwrap()));

        res.push_back(s);
    }
    res
}

impl Peerlist {
    pub fn from(c: &Client) -> Peerlist {
        Peerlist {
            list: c.peer_list.clone(),
            progress: Arc::clone(&c.progress),
            interval: 0,
            port: c.port,
            info_hash: c.torrent.info_hash.clone(),
            peer_id: c.torrent.peer_id.clone(),
            announce: c.torrent.announce.clone(),
        }
    }

    pub async fn poll_peerlist(&mut self) {
        loop {
            println!("Getting peerlist");
            self.get_peerlist().await;
            println!("Got peerlist");
            tokio::time::delay_for(Duration::from_secs(self.interval)).await;
        }
    }

    async fn get_peerlist(&mut self) {
        // manually encode bytes
        let url = format!("{}?info_hash={}&peer_id={}",
                          self.announce,
                          serialize_bytes(&self.info_hash),
                          serialize_bytes(&self.peer_id));
        let mut params = vec![("port", self.port as u64),
                        ("compact", 1)];

        {
            let p = self.progress.lock().await;
            params.extend(vec![("uploaded", p.uploaded as u64),
                            ("downloaded", p.downloaded as u64),
                            ("left", p.left as u64)]);
        }

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

        let res: TrackerResponse = serde_bencode::de::from_bytes(&res)
                                         .expect("Could not parse tracker response!");

        self.list.replace(parse_peerlist(res.peers.as_slice())).await;
        // hack for own tracker
        // self.list.push(format!("localhost:{}", 4444).to_string()).await;
        self.interval = res.interval;
    }
}
