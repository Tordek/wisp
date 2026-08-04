use crate::bus::{Address, Device, Native};

pub struct Memory {
    pub base: Address,
    pub bytes: Vec<u8>,
}
impl Device for Memory {
    fn set_location(&mut self, base_address: Address) {
        self.base = base_address
    }
    fn read_byte(&self, address: Address) -> u8 {
        self.bytes[address.0 as usize]
    }

    fn write_byte(&mut self, address: Address, data: u8) {
        self.bytes[address.0 as usize] = data
    }

    fn read_word(&self, addr: Address) -> Native {
        Native(u64::from_le_bytes(
            self.bytes[(addr.0 as usize)..][..8]
                .try_into()
                .expect("Out of bands memory access."),
        ))
    }

    fn write_word(&mut self, addr: Address, data: Native) {
        self.bytes[(addr.0 as usize)..][..8].copy_from_slice(&data.0.to_le_bytes())
    }
}
