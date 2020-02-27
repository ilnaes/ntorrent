#[derive(Debug)]
pub struct Bitfield {
    pub bf: Vec<u8>,
}

impl Bitfield {
    pub fn has(&self, x: usize) -> bool {
        if self.bf.len() == 0 || self.bf.len() * 8 < x {
            return false;
        }

        let i = x / 8;
        let j = 7 - x % 8;
        return (self.bf[i] >> j) & 1 == 1;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_bf_has() {
        let bf = super::Bitfield { bf: vec![2] };
        assert!(bf.has(6));
        assert!(!bf.has(4));
    }
}
