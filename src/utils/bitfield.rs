#[derive(Debug)]
pub struct Bitfield {
    pub bf: Vec<u8>,
}

impl Bitfield {
    /// returns true if the bitfield has item at location x
    pub fn has(&self, x: usize) -> bool {
        if self.bf.len() == 0 || self.bf.len() * 8 < x {
            return false;
        }

        let i = x / 8;
        let j = 7 - x % 8;
        return (self.bf[i] >> j) & 1 == 1;
    }

    /// switches the bit at location x to 1
    pub fn add(&mut self, x: usize) {
        if self.bf.len() == 0 || self.bf.len() * 8 < x {
            return;
        }

        let i = x / 8;
        let j = 7 - x % 8;
        self.bf[i] = self.bf[i] | (1 << j);
    }

    pub fn len(&self) -> usize {
        self.bf.len()
    }

    pub fn from(bf: Vec<u8>) -> Bitfield {
        Bitfield { bf }
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

    #[test]
    fn test_add() {
        let mut bf = super::Bitfield { bf: vec![0, 0] };
        assert!(!bf.has(2));

        bf.add(2);
        assert!(bf.has(2));

        assert!(!bf.has(10));
        bf.add(10);
        assert!(bf.has(10));
    }
}
