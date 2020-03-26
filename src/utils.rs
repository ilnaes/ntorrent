use crate::consts::BLOCKSIZE;

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
