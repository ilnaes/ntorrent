use crate::client::Progress;
use std::sync::{Arc, Mutex};
use crate::utils::serialize_bytes;
use crate::messages;
use byteorder::{BigEndian, ReadBytesExt};
use std::io::Cursor;

pub struct Peerlist {
    progress: Arc<Mutex<Progress>>,
    list: Vec<String>,
    interval: u64,
    announce: String,
    info_hash: Vec<u8>,
    peer_id: Vec<u8>,
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

impl Peerlist {
    async fn get_peerlist(&mut self) {
        // manually encode bytes
        let url = format!("{}?info_hash={}&peer_id={}",
                          self.announce,
                          serialize_bytes(&self.info_hash),
                          serialize_bytes(&self.peer_id));
        let mut params = vec![("port", self.port),
                        ("compact", 1)];

        {
            let p = self.progress.lock().unwrap();
            params.extend(vec![("uploaded", p.uploaded),
                            ("downloaded", p.downloaded),
                            ("left", p.left)]);
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

        let d: Result<messages::TrackerResponse, serde_bencode::error::Error> = serde_bencode::de::from_bytes(&res);
        if let Ok(f) = d {
            self.interval = f.interval;
            self.list = parse_peerlist(f.peers.to_vec());
            println!("{:?}", self.list);
        } else {
            panic!("Could not parse tracker response!");
        }
    }
}
