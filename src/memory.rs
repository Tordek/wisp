pub trait Memory {
    fn read_word(&self, addr: u64) -> u64;
    fn write_word(&mut self, addr: u64, data: u64);
}
