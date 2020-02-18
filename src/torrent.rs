pub mod torrent_file {
    use serde::{Deserialize, Serialize};
    use serde_bytes::ByteBuf;

    #[derive(Serialize, Deserialize, Debug)]
    pub struct File {
        length: i64,
        path: Vec<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct Info {
        name: String,
        #[serde(rename = "piece length")]
        piece_length: i64,
        pieces: ByteBuf,
        length: Option<i64>,
        files: Option<Vec<File>>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    pub struct TorrentFile {
        announce: String,
        info: Info,
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn t_works() {}
}
