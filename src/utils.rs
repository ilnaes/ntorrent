use crate::consts::BLOCKSIZE;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

pub fn serialize_bytes(b: &Vec<u8>) -> String {
    url::form_urlencoded::byte_serialize(b.as_slice())
        .collect::<Vec<_>>()
        .concat()
}

// takes index, start, and total length of piece,
// returns message for request
pub fn calc_request(i: usize, start: usize, len: usize) -> Option<(Vec<u8>, usize)> {
    let mut res = vec![];
    WriteBytesExt::write_u32::<BigEndian>(&mut res, i as u32).ok()?;
    WriteBytesExt::write_u32::<BigEndian>(&mut res, start as u32).ok()?;

    let mut l = BLOCKSIZE;
    if BLOCKSIZE + start > len {
        l = len - start;
    }
    WriteBytesExt::write_u32::<BigEndian>(&mut res, l as u32).ok()?;
    Some((res, l))
}

pub fn read_piece(msg: Vec<u8>) -> Option<(usize, usize, Vec<u8>)> {
    if msg.len() < 2 {
        return None;
    }

    let (left, right) = msg.split_at(8);
    let mut cx = Cursor::new(left);
    let idx = cx.read_u32::<BigEndian>().ok()? as usize;
    let start = cx.read_u32::<BigEndian>().ok()? as usize;
    Some((idx, start, right.to_vec()))
}
