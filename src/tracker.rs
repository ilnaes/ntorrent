pub mod downloader {
    use reqwest::Client;
    use crate::torrent_file::torrent;
    use crate::messages;
    use byteorder::{BigEndian, ReadBytesExt};
    use std::io::Cursor;
    use async_std::task;
    use std::time::Duration;

    pub struct Downloader {
        torrent: torrent::Torrent,
        interval: u64,
        peer_list: Vec<String>,
        uploaded: i64,
        downloaded: i64,
        left: i64,
        port: i64,
    }

    fn serialize_bytes(b: &Vec<u8>) -> String {
        url::form_urlencoded::byte_serialize(b.as_slice()).collect::<Vec<_>>().concat()
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
            for _ in 0..3 {
                s.push_str(&format!("{}.", cx.read_u8().unwrap()));
            }
            s.pop();
            s.push(':');
            s.push_str(&format!("{}", cx.read_u16::<BigEndian>().unwrap()));

            res.push(s);
        }
        res
    }

    impl Downloader{ 
        pub fn from_file(torrent: torrent::Torrent) -> Downloader {
            Downloader {
                torrent,
                interval: 0,
                peer_list: Vec::new(),
                uploaded: 0,
                downloaded: 0,
                left: 0,
                port: 2222,
            }
        }

        pub async fn download(&mut self) {
            self.get_peerlist().await;
            tokio::join!(self.poll_peerlist());
        }

        async fn poll_peerlist(&mut self) {
            loop {
                task::sleep(Duration::from_secs(self.interval)).await;
                self.get_peerlist().await;
            }
        }

        async fn get_peerlist(&mut self) {
            // manually encode bytes
            let url = format!("{}?info_hash={}&peer_id={}",
                              self.torrent.announce,
                              serialize_bytes(&self.torrent.info_hash),
                              serialize_bytes(&self.torrent.peer_id));
            let params = [("port", self.port),
                            ("uploaded", self.uploaded),
                            ("downloaded", self.downloaded),
                            ("compact", 1),
                            ("left", self.torrent.piece_length)];

            let client = Client::new();
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
}

#[cfg(test)]
mod tests {
    #[test]
    fn peerlist_test() {
    }
}
