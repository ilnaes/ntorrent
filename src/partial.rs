use crate::torrents::Torrent;
use crate::utils::bitfield::Bitfield;
use byteorder::{BigEndian, WriteBytesExt};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct Progress {
    pub uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
}

pub struct Partial<'a> {
    file: Option<File>,
    // filename: String,
    buf: Vec<u8>,
    received: usize,
    torrent: &'a Torrent,
    num_pieces: usize,
    pub progress: Arc<Mutex<Progress>>,
    pub bf: Arc<Mutex<Bitfield>>,
}

impl<'a> Partial<'a> {
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

        let buf = vec![0; torrent.length];

        let mut file_buf = Vec::new();

        if let Some(f) = file.as_mut() {
            if let Ok(_) = f.read_to_end(&mut file_buf) {
                let n = 4 + torrent.piece_length as usize;
                if buf.len() % n == 0 {
                    for _ in 1..(buf.len() / n) {
                        // TODO
                    }
                }
            }
        }

        Partial {
            torrent,
            file,
            buf,
            num_pieces: bf_len,
            received: 0,
            progress: Arc::new(Mutex::new(Progress {
                downloaded: 0,
                uploaded: 0,
                left: torrent.length,
            })),
            bf: Arc::new(Mutex::new(bf)),
        }
    }

    fn write_file(&self) {
        let mut i = 0;

        for f in self.torrent.files.iter() {
            let path = &f.path.join("/");
            println!("Writing to {}", path);
            let path = Path::new(&path);

            // create directories if necessary
            let prefix = path.parent().unwrap();
            std::fs::create_dir_all(prefix).expect("Could not create directories");

            let mut file =
                File::create(&path).expect(&format!("Could not create {}", path.to_str().unwrap()));

            file.write(&self.buf[i..i + f.length])
                .expect("Could not write to file");
            i += f.length;
        }
        println!("FINISHED");
    }

    // updates bitfield and writes piece to file
    // returns None if already has piece
    // returns Some(true) if finished
    pub async fn update(&mut self, idx: u32, res: Vec<u8>) -> Option<bool> {
        let mut bf = self.bf.lock().await;
        // check if already has piece
        if bf.has(idx as usize) {
            return None;
        }

        // write to buffer
        let start = idx as usize * self.torrent.piece_length as usize;
        self.buf[start..start + res.len()].copy_from_slice(res.as_slice());
        self.received += 1;

        // mark bit
        bf.add(idx as usize);

        let mut prog = self.progress.lock().await;
        prog.downloaded += res.len();
        prog.left -= res.len();

        // write to partial file
        if let Some(f) = self.file.as_mut() {
            // TODO: maybe error check this
            f.write_u32::<BigEndian>(idx).ok();
            f.write_all(res.as_slice()).ok();
        }

        if self.received == self.num_pieces {
            self.write_file();
            Some(true)
        } else {
            Some(false)
        }
    }

    // returns byte piece if present
    pub async fn get(&self, idx: u32, offset: u32, len: u32) -> Option<Vec<u8>> {
        let bf = self.bf.lock().await;
        if !bf.has(idx as usize) {
            return None;
        }

        let start = idx as usize * self.torrent.piece_length as usize;
        return Some(
            self.buf[start + offset as usize..start + offset as usize + len as usize].to_vec(),
        );
    }
}
