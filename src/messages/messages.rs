use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

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
    Cancel(u32, u32, u32),
    Port(u16),
}

impl Message {
    pub fn deserialize(buf: &[u8]) -> Option<Message> {
        let mut cx = Cursor::new(buf);
        match cx.read_u8().ok()? {
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
                Some(Message::Piece(i, s, cx.into_inner()[9..].to_vec()))
            }
            8 => {
                let i = cx.read_u32::<BigEndian>().ok()?;
                let s = cx.read_u32::<BigEndian>().ok()?;
                let len = cx.read_u32::<BigEndian>().ok()?;
                Some(Message::Cancel(i, s, len))
            }
            9 => {
                let p = cx.read_u16::<BigEndian>().ok()?;
                Some(Message::Port(p))
            }
            _ => None,
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
        buf

        // let mut res = vec![];
        // WriteBytesExt::write_u32::<BigEndian>(&mut res, buf.len() as u32).unwrap();
        // res.extend(buf);
        // res
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_serialize() {}
}
