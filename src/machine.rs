use crate::{
    cpu::{Cpu, MemoryLayout, SymbolLayout, Word, WordType},
    memory::Memory,
};

type MachineAddressSize = u64;
type WordSize = u64;

#[derive(Clone, Copy)]
struct RawAddress(usize);
#[derive(Clone, Copy)]
struct MachineAddress(MachineAddressSize);

pub struct WispMemory {
    bytes: Vec<u8>,
}

impl Memory<MachineAddressSize, WordSize> for WispMemory {
    fn read_word(&self, position: MachineAddressSize) -> WordSize {
        let start = MachineAddress(position).to_raw();
        let end = MachineAddress(position).offset(1).to_raw();
        let bytes: [u8; WispMemory::WORD_SIZE] = self.bytes[start.0..end.0].try_into().unwrap();
        u64::from_le_bytes(bytes)
    }

    fn write_word(&mut self, position: MachineAddressSize, value: WordSize) {
        let start = MachineAddress(position).to_raw();
        let end = MachineAddress(position).offset(1).to_raw();
        self.bytes[start.0..end.0].copy_from_slice(&value.to_le_bytes());
    }
}

impl MachineAddress {
    const fn to_raw(&self) -> RawAddress {
        RawAddress(self.0 as usize * WispMemory::WORD_SIZE)
    }

    const fn offset(&self, offset: MachineAddressSize) -> MachineAddress {
        MachineAddress(self.0 + offset)
    }
}

impl RawAddress {
    const fn to_machine(&self) -> MachineAddress {
        MachineAddress((self.0 / WispMemory::WORD_SIZE) as MachineAddressSize)
    }

    const fn offset(&self, offset: usize) -> RawAddress {
        RawAddress(self.0 + offset)
    }

    const fn offset_aligned(&self, offset: usize) -> RawAddress {
        RawAddress((self.0 + offset).next_multiple_of(8))
    }
}

impl WispMemory {
    const WORD_SIZE: usize = 8;

    const BOOTSTRAP_OBJECTS: MachineAddress = MachineAddress(MemoryLayout::CPU_RESERVED_END);
    const BOOTSTRAP_LOCATION: MachineAddress =
        MachineAddress(MemoryLayout::CPU_RESERVED_END.next_multiple_of(0x10000));

    // fn read_bytes(&self, start: usize, len: usize) -> &[u8] {
    //     &self.bytes[start..start + len]
    // }

    fn write_bytes(&mut self, start: RawAddress, bytes: &[u8]) {
        self.bytes[start.0..start.offset(bytes.len()).0].copy_from_slice(bytes);
    }

    pub fn new(size: usize) -> Self {
        let mut memory = Self {
            bytes: vec![0; size],
        };
        let mut allocator = ObjectAllocator::new(Self::BOOTSTRAP_OBJECTS, &mut memory);
        let nil_symbol = allocator.write_aligned_symbol("nil", Word::undefined());
        let t_symbol = allocator.write_aligned_symbol("t", Word::undefined());

        let nil = Word::new(WordType::Symbol, nil_symbol.0);
        memory.write_word(MemoryLayout::NIL_ROOT, nil.into());

        let t = Word::new(WordType::Symbol, t_symbol.0);
        memory.write_word(MemoryLayout::T_ROOT, t.into());

        let nil_plist = nil_symbol.offset(MachineAddress(SymbolLayout::PLIST_OFFSET).0);
        let t_plist = t_symbol.offset(MachineAddress(SymbolLayout::PLIST_OFFSET).0);
        memory.write_word(nil_plist.0, nil.into());
        memory.write_word(t_plist.0, nil.into());

        memory.write_word(MemoryLayout::RESET_VECTOR, Self::BOOTSTRAP_LOCATION.0);

        memory.write_word(Self::BOOTSTRAP_LOCATION.0, 0);
        memory.write_word(Self::BOOTSTRAP_LOCATION.offset(1).0, 0);
        memory
    }
}

struct ObjectAllocator<'a> {
    position: MachineAddress,
    memory: &'a mut WispMemory,
}
impl<'a> ObjectAllocator<'a> {
    fn write_aligned(&mut self, data: &[u8]) -> MachineAddress {
        let start = self.position;
        self.memory.write_bytes(self.position.to_raw(), data);
        self.position = self
            .position
            .to_raw()
            .offset_aligned(data.len())
            .to_machine();
        start
    }

    fn write_aligned_string(&mut self, data: &str) -> MachineAddress {
        let len = data.len() as u64;
        let start = self.write_aligned(&len.to_le_bytes());
        self.write_aligned(data.as_bytes());
        start
    }

    fn write_aligned_symbol(&mut self, name: &str, plist: Word) -> MachineAddress {
        let name_address = self.write_aligned_string(name);
        let symbol_address = self.write_aligned(&name_address.0.to_le_bytes());
        self.write_aligned(&(u64::from(plist)).to_le_bytes());
        symbol_address
    }

    fn new(start: MachineAddress, memory: &'a mut WispMemory) -> Self {
        Self {
            position: start,
            memory,
        }
    }
}

pub struct WispMachine {
    cpu: Cpu,

    ram: WispMemory,

    pub halted: bool,
}

impl WispMachine {
    pub fn reset(&mut self) {
        self.cpu.reset(&self.ram);
    }

    pub fn step(&mut self) {
        self.cpu.step(&mut self.ram);
        if self.cpu.halted {
            self.halted = true;
        }
    }

    pub fn new(cpu: Cpu, ram: WispMemory) -> Self {
        Self {
            cpu,
            ram,
            halted: false,
        }
    }
}
