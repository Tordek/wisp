pub trait Memory<AddressSize, WordSize> {
    fn read_word(&self, addr: AddressSize) -> WordSize;
    fn write_word(&mut self, addr: AddressSize, data: WordSize);
}
