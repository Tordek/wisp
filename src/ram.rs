use crate::{
    bus::{Address, Device, Native},
    rom::ROM,
};

pub struct RAM {
    pub ram_device: Memory,
    pub configuration_device: ROM,
}

pub struct Memory {
    pub bytes: Vec<u8>,
}

impl RAM {
    pub fn new(size: usize) -> Self {
        let mut romdata = [
            vec![
                0xaa, 0x55, 't' as u8, 'o' as u8, 'r' as u8, 'd' as u8, 'e' as u8, 'k' as u8, 0, 0,
                0, 0, 0, 0, 0, 0,
            ],
            size.to_le_bytes().to_vec(),
        ]
        .concat();
        romdata.resize(32, 0);
        Self {
            ram_device: Memory {
                bytes: vec![0; size],
            },
            configuration_device: ROM::new(romdata),
        }
    }
}

impl Device for Memory {
    fn set_location(&mut self, _: Address) {}

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
