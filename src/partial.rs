use crate::torrents::Torrent;
use crate::utils::bitfield::Bitfield;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::collections::VecDeque;
use std::fs::{remove_file, File, OpenOptions};
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
    filename: String,
    buf: Vec<u8>,
    received: usize,
    torrent: &'a Torrent,
    num_pieces: usize,
    pub progress: Arc<Mutex<Progress>>,
    pub bf: Arc<Mutex<Bitfield>>,
    pub done: bool,
}

impl<'a> Partial<'a> {
    pub fn from(torrent: &'a Torrent, dir: &str) -> Partial<'a> {
        let filename = if dir == "" {
            format!("{}{}", torrent.name, ".part")
        } else {
            format!("{}/{}{}", dir, torrent.name, ".part")
        };

        let bf_len = (torrent.length - 1) / (8 * torrent.piece_length as usize) + 1;
        let bf = Bitfield::new(bf_len);
        let len = (torrent.length - 1) / (torrent.piece_length as usize) + 1;

        Partial {
            torrent,
            file: None,
            filename,
            buf: vec![0; torrent.length],
            num_pieces: len,
            received: 0,
            progress: Arc::new(Mutex::new(Progress {
                downloaded: 0,
                uploaded: 0,
                left: torrent.length,
            })),
            bf: Arc::new(Mutex::new(bf)),
            done: false,
        }
    }

    // determines if there has been progress
    pub async fn recover(&mut self) {
        if Path::new(&self.filename).exists() {
            let file = OpenOptions::new()
                .read(true)
                .append(true)
                .create(true)
                .open(&self.filename)
                .ok();
            // read in .part file
            if self.read_part(file).await != None {
                return;
            }

            // error in reading part file so erase bad file
            remove_file(&self.filename).ok();
        } else if let Some(true) = self.has().await {
            // already have file
            self.done = true;
            return;
        }

        self.file = OpenOptions::new()
            .read(true)
            .append(true)
            .create(true)
            .open(&self.filename)
            .ok();
    }

    // reads a .part file and updates self
    // returns None if anything goes wrong indicating corrupt part file
    async fn read_part(&mut self, mut file: Option<File>) -> Option<()> {
        let f = file.as_mut()?;
        let num_bytes = f.metadata().ok()?.len();
        let n = self.torrent.piece_length as u64;

        if num_bytes % (n + 4) != 0 || num_bytes > (n + 4) * self.torrent.piece_length as u64 {
            return None;
        }

        let bf_len = (self.torrent.length - 1) / (8 * self.torrent.piece_length as usize) + 1;
        let mut bf = Bitfield::new(bf_len);

        let mut buf = vec![0; self.torrent.piece_length as usize];
        let q = self.torrent.pieces.get_q();
        let q = q.lock().await;

        println!("Recovering from {}", self.filename);

        let mut i = 0;
        while i < num_bytes {
            let idx = f.read_u32::<BigEndian>().ok()? as usize;
            let start = idx * self.torrent.piece_length as usize;

            // check if a double piece (would be bad)
            if bf.has(idx) {
                return None;
            }

            bf.add(idx);
            f.read_exact(buf.as_mut_slice()).ok()?;

            // verify
            if !q.get(idx)?.verify(&buf) {
                return None;
            }
            self.buf[start..start + n as usize].copy_from_slice(&buf);

            i += n + 4;
        }
        drop(q);

        // get rid of pieces that we now have
        let mut newq = VecDeque::new();
        for i in 0..self.torrent.pieces.len().await {
            let piece = self.torrent.pieces.pop_block().await;

            if !bf.has(i) {
                newq.push_back(piece);
            }
        }
        self.torrent.pieces.replace(newq).await;

        self.bf = Arc::new(Mutex::new(bf));
        self.file = file;
        self.received = (num_bytes / (n + 4)) as usize;

        Some(())
    }

    // returns None if no files exist
    // Some(true) if all files exist and are verified
    // Some(false) otherwise
    async fn has(&mut self) -> Option<bool> {
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
                            // bad piece
                            return Some(false);
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

                // all pieces verified
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
            self.done = true;
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

        let start = (offset + idx * self.torrent.piece_length) as usize;
        return Some(self.buf[start..start + len as usize].to_vec());
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
        remove_file(self.filename.clone()).ok();
        println!("FINISHED");
    }
}
