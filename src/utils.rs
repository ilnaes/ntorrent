use crate::consts::BLOCKSIZE;
// use byteorder::{BigEndian, ReadBytesExt};
// use std::io::Cursor;

pub mod bitfield;
pub mod queue;

pub fn serialize_bytes(b: &Vec<u8>) -> String {
    url::form_urlencoded::byte_serialize(b.as_slice())
        .collect::<Vec<_>>()
        .concat()
}

// takes start, and total length of piece,
// returns message for request
pub fn calc_request(start: u32, len: u32) -> u32 {
    let mut l = BLOCKSIZE;
    if BLOCKSIZE + start > len {
        l = len - start;
    }
    l
}

// pub fn read_piece(msg: Vec<u8>) -> Option<(u32, u32, Vec<u8>)> {
//     if msg.len() < 2 {
//         return None;
//     }

//     let (left, right) = msg.split_at(8);
//     let mut cx = Cursor::new(left);
//     let idx = cx.read_u32::<BigEndian>().ok()?;
//     let start = cx.read_u32::<BigEndian>().ok()?;
//     Some((idx, start, right.to_vec()))
// }
