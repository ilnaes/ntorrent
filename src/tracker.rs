pub mod downloader {
    use reqwest::Client;
    use serde::{Deserialize, Serialize};
    use crate::torrent_file::torrent;

    pub struct Downloader {
        torrent: torrent::Torrent,
        interval: i64,
        peer_list: Vec<String>,
        uploaded: i64,
        downloaded: i64,
        left: i64,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct TrackerResponse {
        interval: i64,
        peers: Vec<String>,
    }

    fn serialize_bytes(b: &Vec<u8>) -> String {
        url::form_urlencoded::byte_serialize(b.as_slice()).collect::<Vec<_>>().concat()
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
            }
        }

        pub async fn download(&mut self) {
            self.get_peerlist().await;
        }

        async fn get_peerlist(&mut self) {
            let client = Client::new();
            let url = format!("{}?info_hash={}&peer_id={}", self.torrent.announce, serialize_bytes(&self.torrent.info_hash), serialize_bytes(&self.torrent.peer_id));
            let param = [("port", String::from("2222")), ("uploaded", String::from("0")), ("downloaded", String::from("0")), ("compact", String::from("1")), ("left", String::from(format!("{}", self.torrent.piece_length)))];
            let res = client
                .get(url.as_str())
                .query(&param)
                .send()
                .await;

            if let Ok(r) = res {
                let res = r.text().await.unwrap();
                println!("{}", res);

                let d: Result<TrackerResponse, serde_bencode::error::Error> = serde_bencode::de::from_bytes(res.as_bytes());
            }
        }
    }
}
