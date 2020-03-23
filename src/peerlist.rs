use crate::client::Progress;
use crate::utils::serialize_bytes;
use crate::client::Client;
use crate::messages::messages::TrackerResponse;
use crate::messages::ops;
use crate::utils::queue::WorkQueue;
use tokio::sync::{Mutex, broadcast};
use std::sync::Arc;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;
use std::time::Duration;
use std::collections::VecDeque;

pub struct Peerlist {
    progress: Arc<Mutex<Progress>>,
    interval: u64,
    announce: String,
    info_hash: Vec<u8>,
    peer_id: Vec<u8>,
    port: i64,
    list: WorkQueue<String>,
}

fn parse_peerlist(buf: Vec<u8>) -> VecDeque<String> {
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

    pub async fn poll_peerlist(&mut self, mut rtx: broadcast::Receiver<ops::Op>) {
        let mut exit = false;
        while !exit {
            println!("Getting peerlist");
            self.get_peerlist().await;
            println!("Got peerlist");
            loop {
                tokio::select! {
                    _ = rtx.recv() => {
                        exit = true;
                    },
                    _ = tokio::time::delay_for(Duration::from_secs(self.interval)) => {
                    },
                }
                break;
            }
        }
    }

    async fn get_peerlist(&mut self) {
        // manually encode bytes
        let url = format!("{}?info_hash={}&peer_id={}",
                          self.announce,
                          serialize_bytes(&self.info_hash),
                          serialize_bytes(&self.peer_id));
        let mut params = vec![("port", self.port),
                        ("compact", 1)];

        {
            let p = self.progress.lock().await;
            params.extend(vec![("uploaded", p.uploaded as i64),
                            ("downloaded", p.downloaded as i64),
                            ("left", p.left as i64)]);
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

        self.list.replace(parse_peerlist(res.peers.to_vec())).await;
        // hack for own tracker
        self.list.push("localhost:54331".to_string()).await;
        self.interval = res.interval;
    }
}
