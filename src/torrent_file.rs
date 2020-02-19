pub mod torrent {
    use crypto::digest::Digest;
    use crypto::sha1::Sha1;
    use serde::{Deserialize, Serialize};
    use serde_bytes::ByteBuf;
    use std::fs;

    #[derive(Serialize, Deserialize, Debug)]
    struct FileInfo {
        pub length: i64,
        pub path: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct Info {
        #[serde(default)]
        files: Option<Vec<FileInfo>>,
        #[serde(default)]
        length: Option<i64>,
        pub name: String,
        #[serde(rename = "piece length")]
        pub piece_length: i64,
        pieces: ByteBuf,
    }

    #[derive(Serialize, Deserialize, Debug)]
    struct TorrentFile {
        pub announce: String,
        pub info: Info,
    }

    impl TorrentFile {
        fn new(s: &str) -> Result<TorrentFile, &str> {
            let contents = fs::read(&s).unwrap();
            match serde_bencode::de::from_bytes(contents.as_slice()) {
                Ok(t) => Ok(t),
                Err(_) => Err("Could not open"),
            }
        }
    }

    struct File {
        length: i64,
        path: Vec<String>,
    }

    pub struct Torrent {
        announce: String,
        name: String,
        piece_length: i64,
        info_hash: String,
        pieces: Vec<[u8; 20]>,
        // files: Vec<File>,
    }

    fn split_hash(pieces: Vec<u8>) -> Vec<[u8; 20]> {
        let num_pieces = (pieces.len() / 20) + 1;
        let mut res: Vec<[u8; 20]> = Vec::with_capacity(num_pieces);
        let arr = pieces.as_slice();

        for i in 0..num_pieces {
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

            Ok(Torrent {
                announce: f.announce,
                name: f.info.name,
                piece_length: f.info.piece_length,
                info_hash: hash.result_str(),
                pieces: split_hash(f.info.pieces.into_vec()),
            })
        } else {
            Err("Can't parse torrent file")
        }
    }

    impl Torrent {
        pub fn download(&self) {}
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn what() {}
}
