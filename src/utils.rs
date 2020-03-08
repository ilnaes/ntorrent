use crate::consts::BLOCKSIZE;
use byteorder::{BigEndian, ReadBytesExt, WriteBytesExt};
use std::io::Cursor;

pub fn serialize_bytes(b: &Vec<u8>) -> String {
    url::form_urlencoded::byte_serialize(b.as_slice())
        .collect::<Vec<_>>()
        .concat()
}

pub fn calc_request(i: usize, start: usize, len: usize) -> Option<Vec<u8>> {
    let mut res = vec![];
    WriteBytesExt::write_u32::<BigEndian>(&mut res, i as u32).ok()?;
    WriteBytesExt::write_u32::<BigEndian>(&mut res, (start * BLOCKSIZE) as u32).ok()?;

    let mut l = BLOCKSIZE;
    if l + start * BLOCKSIZE > len {
        l = len - start * BLOCKSIZE;
    }
    WriteBytesExt::write_u32::<BigEndian>(&mut res, l as u32).ok()?;
    Some(res)
}

pub fn read_piece(msg: Vec<u8>) -> Option<(usize, usize)> {
    let mut cx = Cursor::new(msg);
    let idx = cx.read_u32::<BigEndian>().ok()? as usize;
    let start = cx.read_u32::<BigEndian>().ok()? as usize;
    Some((idx, start))
}
