use crate::queue::WorkQueue;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::fs;

#[derive(Serialize, Deserialize, Debug)]
pub struct FileInfo {
    pub length: usize,
    pub path: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Info {
    #[serde(default)]
    files: Option<Vec<FileInfo>>,
    #[serde(default)]
    pub length: Option<usize>,
    pub name: String,
    #[serde(rename = "piece length")]
    pub piece_length: usize,
    pub pieces: ByteBuf,
}

#[derive(Serialize, Deserialize, Debug)]
struct TorrentFile {
    pub announce: String,
    pub info: Info,
}

impl TorrentFile {
    fn new(s: &str) -> TorrentFile {
        let contents = fs::read(&s).expect("Could not read file!");
        serde_bencode::de::from_bytes(contents.as_slice()).expect("Could not decode torrent file!")
    }
}

pub type Piece = ([u8; 20], usize, usize);

pub struct Torrent {
    pub announce: String,
    pub piece_length: usize,
    pub info_hash: Vec<u8>,
    pub pieces: WorkQueue<Piece>,
    pub files: Vec<FileInfo>,
    pub peer_id: Vec<u8>,
}

pub fn split_hash(pieces: Vec<u8>, piece_length: usize, length: usize) -> VecDeque<Piece> {
    let num_pieces = ((pieces.len() - 1) / 20) + 1;
    if num_pieces * piece_length < length || num_pieces * piece_length >= length + piece_length {
        panic!("Bad length");
    }

    let mut res: VecDeque<Piece> = VecDeque::with_capacity(num_pieces);
    let arr = pieces.as_slice();

    for i in 0..num_pieces {
        let mut new = [0; 20];
        let j = 20 * i;
        for k in 0..20 {
            if j + k > pieces.len() - 1 {
                break;
            }
            new[k] = arr[j + k]
        }

        if i == num_pieces - 1 && length % piece_length != 0 {
            res.push_back((new, i, length % piece_length));
        } else {
            res.push_back((new, i, piece_length));
        }
    }
    res
}

impl Torrent {
    pub fn new(s: &str) -> Torrent {
        let f = TorrentFile::new(s);

        // calculate info hash
        let mut hash = Sha1::new();
        let info = serde_bencode::to_bytes(&f.info).expect("Could not encode info hash!");
        hash.input(info.as_slice());

        // if only one file, create new FileInfo
        let files = f.info.files.unwrap_or({
            vec![FileInfo {
                length: f.info.length.unwrap(),
                path: vec![f.info.name],
            }]
        });

        let size = files.iter().map(|x| x.length).fold(0, |a, b| a + b);

        // randomly generate id
        let id: [u8; 20] = rand::random();

        println!("Piece length: {}", f.info.piece_length);

        Torrent {
            announce: f.announce,
            piece_length: f.info.piece_length,
            info_hash: hash.result().as_slice().to_vec(),
            pieces: WorkQueue::from(split_hash(
                f.info.pieces.into_vec(),
                f.info.piece_length,
                size,
            )),
            files,
            peer_id: id.as_ref().to_vec(),
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_split() {
        let r = super::split_hash(vec![1, 2, 3], 4, 4);
        assert_eq!(
            r[0],
            (
                [1, 2, 3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                0,
                4
            )
        );

        let r = super::split_hash(vec![0; 39], 4, 8);
        assert_eq!(r.len(), 2);
        assert_eq!(r[1].0.len(), 20);

        let r = super::split_hash(vec![0; 41], 4, 10);
        assert_eq!(r.len(), 3);
        assert_eq!(r[2].2, 2);
    }

    #[test]
    #[should_panic]
    fn test_panic() {
        super::split_hash(vec![0; 40], 2, 5);
    }
}
