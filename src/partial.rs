use crate::utils::bitfield::Bitfield;
use std::fs::File;

pub struct Partial {
    file: Option<File>,
    pub bf: Bitfield,
}

impl Partial {}
