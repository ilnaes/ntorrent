use crate::torrents::Torrent;
use crate::utils::bitfield::Bitfield;
use byteorder::{BigEndian, WriteBytesExt};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Partial {
    file: Option<File>,
    filename: String,
    pub bf: Arc<Mutex<Bitfield>>,
}

impl Partial {
    pub fn from(torrent: &Torrent) -> Partial {
        let filename = format!("{}{}", torrent.name, ".part");
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(filename.clone())
            .ok();

        let bf_len = (torrent.length - 1) / (8 * torrent.piece_length as usize) + 1;
        let bf = Bitfield::new(bf_len);

        let mut buf = Vec::new();

        if let Some(f) = file.as_mut() {
            if let Ok(_) = f.read_to_end(&mut buf) {
                let n = 4 + torrent.piece_length as usize;
                if buf.len() % n == 0 {
                    for _ in 1..(buf.len() / n) {
                        // TODO
                    }
                }
            }
        }

        Partial {
            file,
            filename,
            bf: Arc::new(Mutex::new(bf)),
        }
    }

    pub async fn delete(&mut self) {
        self.file = None;
        // TODO
    }

    // updates bitfield and writes piece to file
    pub async fn update(&mut self, idx: u32, res: Vec<u8>) -> bool {
        // check if already has piece
        let mut bf = self.bf.lock().await;
        if bf.has(idx as usize) {
            return false;
        }
        bf.add(idx as usize);
        drop(bf);

        if let Some(f) = self.file.as_mut() {
            if let Err(_) = f.write_u32::<BigEndian>(idx) {
                return false;
            }
            if let Ok(_) = f.write_all(res.as_slice()) {
                return true;
            } else {
                return false;
            }
        } else {
            return true;
        }
    }
}
