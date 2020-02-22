pub mod torrent {
    use crate::tracker;
    use sha1::{Sha1, Digest};
    use serde::{Deserialize, Serialize};
    use serde_bytes::ByteBuf;
    use std::fs;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct FileInfo {
        pub length: i64,
        pub path: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Info {
        #[serde(default)]
        files: Option<Vec<FileInfo>>,
        #[serde(default)]
        pub length: Option<i64>,
        pub name: String,
        #[serde(rename = "piece length")]
        pub piece_length: i64,
        pub pieces: ByteBuf,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct TorrentFile {
        pub announce: String,
        pub info: Info,
    }

    impl TorrentFile {
        fn new(s: &str) -> Result<TorrentFile, serde_bencode::error::Error> {
            let contents = fs::read(&s).unwrap();
            serde_bencode::de::from_bytes(contents.as_slice())
        }
    }

    pub struct Torrent {
        pub announce: String,
        pub piece_length: i64,
        pub info_hash: Vec<u8>,
        pub pieces: Vec<[u8; 20]>,
        pub files: Vec<FileInfo>,
        pub peer_id: Vec<u8>,
    }

    pub fn split_hash(pieces: Vec<u8>) -> Vec<[u8; 20]> {
        let num_pieces = (pieces.len() / 20) + 1;
        let mut res: Vec<[u8; 20]> = Vec::with_capacity(num_pieces);
        let arr = pieces.as_slice();

        println!("{}", num_pieces);

        for i in 0..num_pieces {
            res.push([0; 20]);
            let j = 20 * i;
            for k in 0..20 {
                if j + k > pieces.len() - 1 {
                    break;
                }
                res[i][k] = arr[j + k]
            }
        }
        res
    }

    pub fn new(s: &str) -> Result<Torrent, &str> {
        if let Ok(f) = TorrentFile::new(s) {
            // calculate info hash
            let mut hash = Sha1::new();
            let info = serde_bencode::to_bytes(&f.info).unwrap();
            hash.input(info.as_slice());

            // if only one file, create new FileInfo
            let files = f.info.files.unwrap_or({
                vec![FileInfo {
                    length: f.info.length.unwrap(),
                    path: vec![f.info.name],
                }]
            });

            let id: [u8; 20] = rand::random();

            Ok(Torrent {
                announce: f.announce,
                piece_length: f.info.piece_length,
                info_hash: hash.result().as_slice().to_vec(),
                pieces: split_hash(f.info.pieces.into_vec()),
                files,
                peer_id: id.as_ref().to_vec(),
            })
        } else {
            Err("Can't parse torrent file")
        }
    }

    impl Torrent {
        pub async fn download(self) {
            let mut r = tracker::downloader::Downloader::from_file(self);
            r.download().await;
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_split() {
        use super::torrent;

        let r = torrent::split_hash(vec![1, 2, 3]);
        assert_eq!(
            r[0],
            [1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
        );

        let r = torrent::split_hash(vec![0; 39]);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].len(), 20);

        let r = torrent::split_hash(vec![0; 41]);
        assert_eq!(r.len(), 3);
    }
}
