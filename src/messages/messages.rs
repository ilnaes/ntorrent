use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::io::Cursor;

#[derive(Serialize, Deserialize, Debug)]
pub struct TrackerResponse {
    pub interval: u64,
    pub peers: ByteBuf,
}

#[derive(PartialEq, Debug, Clone)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Bitfield(Vec<u8>),
    Request(u32, u32, u32),
    Have(u32),
    Piece(u32, u32, Vec<u8>),
    // Cancel(u32, u32, u32),
    // Port,
}

impl Message {
    pub fn deserialize(buf: Vec<u8>) -> Option<Message> {
        print!("DER");
        let mut cx = Cursor::new(buf);
        let x = cx.read_u8().ok()?;
        print!("{} ", x);
        match x {
            0 => Some(Message::Choke),
            1 => Some(Message::Unchoke),
            2 => Some(Message::Interested),
            3 => Some(Message::NotInterested),
            4 => {
                let i = cx.read_u32::<BigEndian>().ok()?;
                Some(Message::Have(i))
            }
            5 => Some(Message::Bitfield(cx.into_inner()[1..].to_vec())),
            6 => {
                let i = cx.read_u32::<BigEndian>().ok()?;
                let s = cx.read_u32::<BigEndian>().ok()?;
                let len = cx.read_u32::<BigEndian>().ok()?;
                Some(Message::Request(i, s, len))
            }
            7 => {
                let i = cx.read_u32::<BigEndian>().ok()?;
                let s = cx.read_u32::<BigEndian>().ok()?;
                println!("Piece {} {}", i, s);
                Some(Message::Piece(i, s, cx.into_inner()[9..].to_vec()))
            }
            _ => panic!("BAD DESERIALIZE"),
        }
    }

    // serializes message to byte array
    // consumes self
    pub fn serialize(self) -> Vec<u8> {
        let mut buf = vec![];
        match self {
            Message::Choke => {
                WriteBytesExt::write_u8(&mut buf, 0).unwrap();
            }
            Message::Unchoke => {
                WriteBytesExt::write_u8(&mut buf, 1).unwrap();
            }
            Message::Interested => {
                WriteBytesExt::write_u8(&mut buf, 2).unwrap();
            }
            Message::NotInterested => {
                WriteBytesExt::write_u8(&mut buf, 3).unwrap();
            }
            Message::Have(i) => {
                WriteBytesExt::write_u8(&mut buf, 4).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, i).unwrap();
            }
            Message::Bitfield(v) => {
                WriteBytesExt::write_u8(&mut buf, 5).unwrap();
                buf.extend(v);
            }
            Message::Request(i, s, len) => {
                WriteBytesExt::write_u8(&mut buf, 6).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, i).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, s).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, len).unwrap();
            }
            Message::Piece(i, s, payload) => {
                WriteBytesExt::write_u8(&mut buf, 7).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, i).unwrap();
                WriteBytesExt::write_u32::<BigEndian>(&mut buf, s).unwrap();
                buf.extend(payload)
            }
            _ => {}
        }
        let mut res = vec![];
        WriteBytesExt::write_u32::<BigEndian>(&mut res, buf.len() as u32).unwrap();
        res.extend(buf);
        res
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_serialize() {}
}
