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

pub struct Memory {
    pub bytes: Vec<u8>,
}
