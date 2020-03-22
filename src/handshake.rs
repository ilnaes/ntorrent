use crate::torrents::Torrent;

pub struct Handshake {
    pub info_hash: Vec<u8>,
    pub peer_id: Vec<u8>,
}

impl Handshake {
    pub fn from(t: &Torrent) -> Handshake {
        Handshake {
            info_hash: t.info_hash.clone(),
            peer_id: t.peer_id.clone(),
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut res = vec![19u8];
        res.extend("BitTorrent protocol".as_bytes());
        res.extend(vec![0; 8]);
        res.extend(self.info_hash.clone());
        res.extend(self.peer_id.clone());
        res
    }

    pub fn deserialize(msg: Vec<u8>) -> Option<Handshake> {
        let buf = msg.as_slice();
        if buf[0] != 19 {
            return None;
        }

        if let Ok(s) = String::from_utf8(buf[1..20].to_vec()) {
            if s != "BitTorrent protocol" {
                return None;
            }
        } else {
            return None;
        }

        Some(Handshake {
            info_hash: buf[28..48].to_vec(),
            peer_id: buf[48..68].to_vec(),
        })
    }
}
