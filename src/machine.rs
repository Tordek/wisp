use crate::{
    cpu::{Cpu, MemoryLayout, SymbolLayout, Word, WordType},
    memory::Memory,
};

pub struct WispMemory {
    bytes: Vec<u8>,
}

impl Memory for WispMemory {
    fn read_word(&self, position: u64) -> u64 {
        let start = WispMemory::word_to_bytes(position);
        let end = WispMemory::word_to_bytes(position + 1);
        let bytes: [u8; 8] = self.bytes[start..end].try_into().unwrap();
        u64::from_le_bytes(bytes)
    }

    fn write_word(&mut self, position: u64, value: u64) {
        let start = WispMemory::word_to_bytes(position);
        let end = WispMemory::word_to_bytes(position + 1);
        self.bytes[start..end].copy_from_slice(&value.to_le_bytes());
    }
}

impl WispMemory {
    const WORD_SIZE: u64 = 8;

    const BOOTSTRAP_OBJECTS: usize = WispMemory::word_to_bytes(MemoryLayout::CPU_RESERVED_END);
    const BOOTSTRAP_LOCATION: usize =
        WispMemory::word_to_bytes(MemoryLayout::CPU_RESERVED_END + 0x10000);

    const fn word_to_bytes(position: u64) -> usize {
        (position * WispMemory::WORD_SIZE) as usize
    }

    const fn bytes_to_words(position: usize) -> u64 {
        position as u64 / WispMemory::WORD_SIZE
    }

    // fn read_bytes(&self, start: usize, len: usize) -> &[u8] {
    //     &self.bytes[start..start + len]
    // }

    fn write_bytes(&mut self, start: usize, bytes: &[u8]) {
        self.bytes[start..start + bytes.len()].copy_from_slice(bytes);
    }

    pub fn new(size: usize) -> Self {
        let mut memory = Self {
            bytes: vec![0; size],
        };
        let mut allocator = ObjectAllocator::new(Self::BOOTSTRAP_OBJECTS, &mut memory);
        let nil_symbol = allocator.write_aligned_symbol("nil", Word::undefined());
        let t_symbol = allocator.write_aligned_symbol("t", Word::undefined());

        let nil = Word::new(WordType::Symbol, WispMemory::bytes_to_words(nil_symbol));
        memory.write_word(MemoryLayout::NIL_ROOT, nil.into());

        let t = Word::new(WordType::Symbol, WispMemory::bytes_to_words(t_symbol));
        memory.write_word(MemoryLayout::T_ROOT, t.into());

        let nil_plist = nil_symbol + WispMemory::word_to_bytes(SymbolLayout::PLIST_OFFSET);
        let t_plist = t_symbol + WispMemory::word_to_bytes(SymbolLayout::PLIST_OFFSET);
        memory.write_word(WispMemory::bytes_to_words(nil_plist), nil.into());
        memory.write_word(WispMemory::bytes_to_words(t_plist), nil.into());

        memory.write_word(
            MemoryLayout::RESET_VECTOR,
            WispMemory::bytes_to_words(Self::BOOTSTRAP_LOCATION),
        );

        memory.write_word(WispMemory::bytes_to_words(Self::BOOTSTRAP_LOCATION), 0);
        memory.write_word(WispMemory::bytes_to_words(Self::BOOTSTRAP_LOCATION) + 1, 0);
        memory
    }
}

struct ObjectAllocator<'a> {
    position: usize,
    memory: &'a mut WispMemory,
}
impl<'a> ObjectAllocator<'a> {
    const ALIGNMENT_MASK: usize = 0xffff_ffff_ffff_fff8;

    fn write_aligned(&mut self, data: &[u8]) -> usize {
        let start = self.position;
        self.memory.write_bytes(self.position, data);
        self.position = (self.position + data.len() + 7) & ObjectAllocator::ALIGNMENT_MASK;
        start
    }

    fn write_aligned_string(&mut self, data: &str) -> usize {
        let len = data.len() as u64;
        let start = self.write_aligned(&len.to_le_bytes());
        self.write_aligned(data.as_bytes());
        start
    }

    fn write_aligned_symbol(&mut self, name: &str, plist: Word) -> usize {
        let name_address = self.write_aligned_string(name);
        let symbol_address = self.write_aligned(&name_address.to_le_bytes());
        self.write_aligned(&(u64::from(plist)).to_le_bytes());
        symbol_address
    }

    fn new(start: usize, memory: &'a mut WispMemory) -> Self {
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
