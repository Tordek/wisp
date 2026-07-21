use std::ops::Add;

use crate::{
    cpu::{
        AddressRegister, ConsLayout, Cpu, INSTRUCTION_SIZE, Instruction, MemoryLayout, Register,
        SymbolLayout, Word, WordType,
    },
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

struct Cursor<'a> {
    position: MachineAddress,
    memory: &'a mut [u8],
}
impl<'a> Cursor<'a> {
    fn write_word_at(&mut self, address: MachineAddress, data: &u64) {
        self.memory[address.to_raw().0..].copy_from_slice(&data.to_le_bytes());
    }

    fn write_at(&mut self, address: MachineAddress, data: &[u8]) {
        self.memory[address.to_raw().0..].copy_from_slice(data);
    }

    fn write_aligned(&mut self, data: &[u8]) -> MachineAddress {
        let start = self.position;
        self.write_at(self.position, data);
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

    fn write_instructions_at(&mut self, address: MachineAddress, instructions: &[Instruction]) {
        for (idx, instruction) in instructions.iter().enumerate() {
            let (lo, hi) = instruction.encode();
            self.write_at(
                address.offset(idx as u64 * INSTRUCTION_SIZE),
                &[lo.to_le_bytes(), hi.to_le_bytes()].concat(),
            )
        }
    }

    fn new(start: MachineAddress, memory: &'a mut [u8]) -> Self {
        Self {
            position: start,
            memory,
        }
    }
}
struct FirmwareHelper {
    cursor: MachineAddress,
}
impl FirmwareHelper {
    const BOOTSTRAP_HOOK: MachineAddress = MachineAddress(MemoryLayout::CPU_RESERVED_END);
    const BOOTSTRAP_ALLOC_HOOK: MachineAddress = MachineAddress(0x600);
    const BOOTSTRAP_TRAP_HOOK: MachineAddress = MachineAddress(0x800);
    const BOOTSTRAP_OBJECTS: MachineAddress = MachineAddress(0x3000);
    const BOOTSTRAP_STACK_POSITION: MachineAddress = MachineAddress(0x2000); // Grows backwards
    const BOOTSTRAP_SCRATCH_ALLOC: MachineAddress = MachineAddress(0x2000); // Grows forwards

    fn make_firmware() -> [u8; 0x4000] {
        let mut firmware = [0; 0x4000];
        // Starting at 0x3000, place constant symbols.
        let mut cursor = Cursor::new(Self::BOOTSTRAP_OBJECTS, &mut firmware);
        let nil_symbol = cursor.write_aligned_symbol("nil", Word::undefined());
        let t_symbol = cursor.write_aligned_symbol("t", Word::undefined());

        // At 0x0000, place root objects.
        let nil = Word::new(WordType::Symbol, nil_symbol.0);
        cursor.write_word_at(MachineAddress(MemoryLayout::NIL_ROOT), &nil.into());

        let t = Word::new(WordType::Symbol, t_symbol.0);
        cursor.write_word_at(MachineAddress(MemoryLayout::T_ROOT), &t.into());

        // Finalize objects
        let nil_plist = nil_symbol.offset(MachineAddress(SymbolLayout::PLIST_OFFSET).0);
        cursor.write_word_at(nil_plist, &nil.into());
        let t_plist = t_symbol.offset(MachineAddress(SymbolLayout::PLIST_OFFSET).0);
        cursor.write_word_at(t_plist, &nil.into());

        // Set Vectors.
        cursor.write_word_at(
            MachineAddress(MemoryLayout::RESET_VECTOR),
            &Self::BOOTSTRAP_HOOK.0,
        );
        cursor.write_word_at(
            MachineAddress(MemoryLayout::ALLOC_VECTOR),
            &Self::BOOTSTRAP_ALLOC_HOOK.0,
        );
        cursor.write_word_at(
            MachineAddress(MemoryLayout::ALLOC_CONS_VECTOR),
            &Self::BOOTSTRAP_ALLOC_HOOK.0,
        );
        cursor.write_word_at(
            MachineAddress(MemoryLayout::ALLOC_CONS_VECTOR),
            &Self::BOOTSTRAP_TRAP_HOOK.0,
        );

        // Bootstrap program:
        cursor.write_instructions_at(
            Self::BOOTSTRAP_HOOK,
            &[
                // Set Stack Pointer at 0x2000
                Instruction::LoadAddress {
                    dst: AddressRegister(0),
                    address: Self::BOOTSTRAP_STACK_POSITION.0,
                },
                Instruction::StoreSp {
                    src: AddressRegister(0),
                },
                // Halt
                Instruction::Halt,
            ],
        );

        // Default allocator:
        cursor.write_instructions_at(
            Self::BOOTSTRAP_ALLOC_HOOK,
            &[
                // Store prototype
                Instruction::PopR { dst: Register(0) },
                // Read Free pointer
                Instruction::LoadAddress {
                    dst: AddressRegister(2),
                    address: 0x2fff,
                },
                Instruction::ReadOffsetAdr {
                    dst: AddressRegister(1),
                    base: AddressRegister(2),
                    offset: 0,
                }, // A0 = *freeptr;
                // Advance free pointer
                Instruction::ReadOffsetAdr {
                    dst: AddressRegister(1),
                    base: AddressRegister(2),
                    offset: 0,
                }, // A1 = *freeptr;
                Instruction::PopR { dst: Register(1) }, // R1 = Size
                Instruction::LoadPayload {
                    dst: AddressRegister(3),
                    src: Register(1),
                }, // A3 = R1.payload
                Instruction::AddA {
                    dst: AddressRegister(1),
                    op1: AddressRegister(1),
                    op2: AddressRegister(3),
                }, // A1 += A3;
                Instruction::StoreA { dst: A2, value: A1 }, // *freeptr = A1
                Instruction::PopR { dst: Register(1) }, // R1 = car
                Instruction::StoreOffset {
                    base: AddressRegister(0),
                    offset: ConsLayout::CAR_OFFSET as i64,
                    value: Register(1),
                }, // *A0 = CAR
                Instruction::PopR { dst: Register(2) }, // R1 = car
                Instruction::StoreOffset {
                    base: AddressRegister(0),
                    offset: ConsLayout::CDR_OFFSET as i64,
                    value: Register(1),
                }, // *(A0+1) = CDR
                // TODO: Restore A1, A2, R1, R2.
                Instruction::Return,
            ],
        );

        // Default trap
        cursor.write_instructions_at(Self::BOOTSTRAP_TRAP_HOOK, &[Instruction::Halt]);

        firmware
    }
}

impl WispMemory {
    const WORD_SIZE: usize = 8;

    fn write_bytes(&mut self, start: RawAddress, bytes: &[u8]) {
        self.bytes[start.0..start.offset(bytes.len()).0].copy_from_slice(bytes);
    }

    pub fn new(size: usize) -> Self {
        Self {
            bytes: vec![0; size],
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
        self.cpu.full_step(&mut self.ram);
        if self.cpu.halted {
            self.halted = true;
        }
    }

    pub fn new(cpu: Cpu, ram: WispMemory) -> Self {
        let mut machine = Self {
            cpu,
            ram,
            halted: false,
        };

        machine
            .ram
            .write_bytes(RawAddress(0), &FirmwareHelper::make_firmware());
        machine
    }
}
