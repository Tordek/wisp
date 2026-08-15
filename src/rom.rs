use crate::bus::{Address, Device, Native};

pub struct Rom {
    pub bytes: Vec<u8>,
}

impl Rom {
    pub fn new(data: Vec<u8>) -> Self {
        Rom { bytes: data }
    }
}

impl Device for Rom {
    fn set_location(&mut self, _: Address) {}
    fn read_byte(&self, address: Address) -> u8 {
        self.bytes[address.0 as usize]
    }

    fn write_byte(&mut self, _: Address, _: u8) {}

    fn read_word(&self, addr: Address) -> Native {
        Native(u64::from_le_bytes(
            self.bytes[(addr.0 as usize)..][..8]
                .try_into()
                .expect("Out of bands memory access."),
        ))
    }

    fn write_word(&mut self, _: Address, _: Native) {}
}
