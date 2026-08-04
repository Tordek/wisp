#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Address(pub u64);

impl std::ops::Add<Offset> for Address {
    type Output = Address;

    fn add(self, rhs: Offset) -> Address {
        Address(self.0.wrapping_add_signed(rhs.0))
    }
}

impl std::ops::Sub<Offset> for Address {
    type Output = Address;

    fn sub(self, rhs: Offset) -> Address {
        Address(self.0.wrapping_sub_signed(rhs.0))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Offset(pub i64);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Native(pub u64);
impl std::ops::Add<Native> for Native {
    type Output = Native;

    fn add(self, rhs: Native) -> Native {
        Native(self.0 + rhs.0)
    }
}

impl std::ops::Sub<Native> for Native {
    type Output = Native;

    fn sub(self, rhs: Native) -> Native {
        Native(self.0.wrapping_sub_signed(rhs.0 as i64))
    }
}

impl From<u64> for Native {
    fn from(value: u64) -> Self {
        Native(value)
    }
}

impl From<Native> for u64 {
    fn from(value: Native) -> Self {
        value.0
    }
}

impl From<Address> for Native {
    fn from(value: Address) -> Self {
        Native(value.0)
    }
}

impl From<Native> for Address {
    fn from(value: Native) -> Self {
        Address(value.0)
    }
}

pub struct Bus<'a> {
    mappings: Vec<Mapping<'a>>,
}

struct Mapping<'a> {
    range: std::ops::Range<u64>,
    device: Box<dyn Device + 'a>,
}

pub trait Device {
    fn set_location(&mut self, base_address: Address);
    fn read_word(&self, address: Address) -> Native;
    fn write_word(&mut self, address: Address, data: Native);
    fn read_byte(&self, address: Address) -> u8;
    fn write_byte(&mut self, address: Address, data: u8);
}

#[derive(Debug)]
pub enum BusError {
    NoMapping,
}

impl<'a> Bus<'a> {
    pub fn new() -> Self {
        Bus { mappings: vec![] }
    }

    pub fn install(
        &mut self,
        range: std::ops::Range<u64>,
        mut device: Box<dyn Device + 'a>,
    ) -> Result<(), BusError> {
        // TODO: Error if ranges overlap.
        // for Mapping { range, device } in &self.mappings {
        // }
        device.set_location(Address(range.start));
        self.mappings.push(Mapping { range, device });
        Ok(())
    }

    pub fn read_word(&self, Address(a): Address) -> Native {
        for Mapping { range, device } in &self.mappings {
            if range.contains(&a) {
                return device.read_word(Address(a - range.start));
            }
        }
        Native(0x00) // This should be a BusError but uh... yeah
    }

    pub fn write_word(&mut self, Address(a): Address, data: Native) {
        for Mapping { range, device } in &mut self.mappings {
            if range.contains(&a) {
                return device.write_word(Address(a - range.start), data);
            }
        }
    }
    pub fn read_byte(&self, Address(a): Address) -> u8 {
        for Mapping { range, device } in &self.mappings {
            if range.contains(&a) {
                return device.read_byte(Address(a - range.start));
            }
        }
        0x00 // This should be a BusError but uh... yeah
    }
    pub fn write_byte(&mut self, Address(a): Address, data: u8) {
        for Mapping { range, device } in &mut self.mappings {
            if range.contains(&a) {
                return device.write_byte(Address(a - range.start), data);
            }
        }
    }
}
