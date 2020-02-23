pub fn serialize_bytes(b: &Vec<u8>) -> String {
    url::form_urlencoded::byte_serialize(b.as_slice())
        .collect::<Vec<_>>()
        .concat()
}
