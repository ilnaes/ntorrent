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

    // returns None if no files exist
    // Some(true) if all files exist and are verified
    // Some(false) otherwise
    pub async fn has(&mut self) -> Option<bool> {
        let mut all = true;
        let mut exists = false;
        let mut i = 0;
        let mut buf = vec![0; self.torrent.length];

        for f in self.torrent.files.iter() {
            let path = f.path.join("/");
            let path = Path::new(path.as_str());

            // see if path exist
            if path.exists() {
                exists = true;
                let file = std::fs::read(path);
                if let Ok(v) = file {
                    if v.len() == f.length {
                        buf[i..i + f.length as usize].copy_from_slice(v.as_slice());
                    } else {
                        all = false;
                    }
                } else {
                    all = false;
                }
            } else {
                all = false;
            }
            i += f.length;
        }

        if !exists {
            None
        } else {
            if all {
                // verify all pieces
                {
                    let q = self.torrent.pieces.get_q();
                    let q = q.lock().await;
                    let mut iter = q.iter();
                    let mut i: u64 = 0;
                    let len = self.torrent.piece_length as usize;

                    while let Some(piece) = iter.next() {
                        let ceil = std::cmp::min(i as usize + len, self.torrent.length);
                        if !piece.verify(&buf[i as usize..ceil]) {
                            all = false;
                            break;
                        } else {
                            if (i / (len as u64)) % 100 == 99 {
                                println!(
                                    "Verified {:.2}%",
                                    100f64 * (i + len as u64) as f64 / self.torrent.length as f64
                                );
                            }
                        }
                        i += len as u64;
                    }
                }

                self.buf = buf;
                let mut p = self.progress.lock().await;
                p.left = 0;

                let mut bf = self.bf.lock().await;
                let len = bf.len();
                *bf = Bitfield::from(vec![255; len]);

                self.torrent.pieces.clear().await;
            }
            Some(all)
        }
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
}
