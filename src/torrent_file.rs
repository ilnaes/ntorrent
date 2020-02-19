pub mod torrent {
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
        pub name: String,
        #[serde(rename = "piece length")]
        pub piece_length: i64,
        #[serde(default)]
        length: Option<i64>,
        #[serde(default)]
        files: Option<Vec<FileInfo>>,
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
        // files: Vec<File>,
    }

    pub fn new(s: &str) -> Result<Torrent, &str> {
        let f = TorrentFile::new(s)?;

        Ok(Torrent {
            announce: f.announce,
            name: f.info.name,
            piece_length: f.info.piece_length,
        })
    }

    impl Torrent {
        pub fn download(&self) {}
    }
}
