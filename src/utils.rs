use crate::consts::BLOCKSIZE;
use byteorder::{BigEndian, WriteBytesExt};
use std::error::Error;

pub fn serialize_bytes(b: &Vec<u8>) -> String {
    url::form_urlencoded::byte_serialize(b.as_slice())
        .collect::<Vec<_>>()
        .concat()
}

pub fn calc_request(i: usize, start: usize, len: usize) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut res = vec![];
    WriteBytesExt::write_u32::<BigEndian>(&mut res, i as u32).unwrap();
    WriteBytesExt::write_u32::<BigEndian>(&mut res, start as u32).unwrap();

    let mut l = BLOCKSIZE;
    if l + start * BLOCKSIZE > len {
        l = len - start * BLOCKSIZE;
    }
    WriteBytesExt::write_u32::<BigEndian>(&mut res, l as u32).unwrap();
    Ok(res)
}
