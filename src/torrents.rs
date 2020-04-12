use crate::utils::queue::Queue;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use sha1::{Digest, Sha1};
use std::collections::VecDeque;
use std::fs;

// The following three structs are used for
// serde decoding of bencoded torrent files
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
    pub piece_length: u32,
    pub pieces: ByteBuf,
}

impl Info {
    pub fn hash(&self) -> Vec<u8> {
        let mut hash = Sha1::new();
        let info = serde_bencode::to_bytes(&self).expect("Could not encode info hash!");
        hash.input(info.as_slice());
        hash.result().as_slice().to_vec()
    }
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

// hash * index * length
#[derive(PartialEq, Debug, Clone, Copy)]
pub struct Piece(pub [u8; 20], pub u32, pub u32);

impl Piece {
    // verifies that the buf matches the piece's hash
    pub fn verify(&self, buf: &[u8]) -> bool {
        if buf.len() != self.2 as usize {
            return false;
        }

        let mut hash = Sha1::new();
        hash.input(buf);

        return hash.result().as_slice() == self.0;
    }
}

pub struct Torrent {
    pub name: String,
    pub announce: String,
    pub piece_length: u32,
    pub info_hash: Vec<u8>,
    pub pieces: Queue<Piece>,
    pub files: Vec<FileInfo>,
    pub peer_id: Vec<u8>,
    pub length: usize,
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

        // make sure to record proper length of piece
        if i == num_pieces - 1 && length % piece_length != 0 {
            res.push_back(Piece(new, i as u32, (length % piece_length) as u32));
        } else {
            res.push_back(Piece(new, i as u32, piece_length as u32));
        }
    }
    res
}

impl Torrent {
    pub fn new(s: &str, dir: String) -> Torrent {
        let t = TorrentFile::new(s);
        let info_hash = t.info.hash();

        // randomly generate id
        let id: [u8; 20] = rand::random();

        let mut files;
        if let Some(file) = t.info.files {
            files = file;

            // append base dir in multidoc format
            if t.info.name.len() > 0 {
                for f in files.iter_mut() {
                    f.path.insert(0, t.info.name.clone());

                    if dir != "" {
                        f.path.insert(0, dir.clone());
                    }
                }
            }
        } else {
            // if only one file, create new FileInfo
            let mut path = vec![t.info.name.clone()];

            if dir != "" {
                path.insert(0, dir.clone());
            }
            files = vec![FileInfo {
                length: t.info.length.unwrap(),
                path,
            }]
        };

        let length = files.iter().map(|x| x.length).fold(0, |a, b| a + b);

        Torrent {
            name: t.info.name,
            announce: t.announce,
            piece_length: t.info.piece_length,
            info_hash,
            pieces: Queue::from(split_hash(
                t.info.pieces.into_vec(),
                t.info.piece_length as usize,
                length,
            )),
            files,
            peer_id: id.as_ref().to_vec(),
            length,
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
            super::Piece(
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
