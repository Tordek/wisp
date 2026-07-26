pub trait Memory<const N: usize, WordSize> {
    fn read_word(&self, addr: usize) -> WordSize;
    fn write_word(&mut self, addr: usize, data: WordSize);
}

impl Memory<8, u64> for [u8] {
    fn read_word(&self, addr: usize) -> u64 {
        u64::from_le_bytes(
            self[addr..][..8]
                .try_into()
                .expect("Out of bands memory access."),
        )
    }
    fn write_word(&mut self, addr: usize, data: u64) {
        self[addr..][..8].copy_from_slice(&data.to_le_bytes())
    }
}
