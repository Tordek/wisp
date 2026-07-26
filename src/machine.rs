use crate::{
    cpu::{Cpu, Instruction, InterruptTableOffset, MemoryLayout, SymbolLayout, Word},
    memory::Memory,
    parse_asm,
};

struct Cursor {
    position: usize,
}
impl Cursor {
    fn write_at(&mut self, target: &mut [u8], address: usize, data: &[u8]) {
        target[address..address + (data.len())].copy_from_slice(data);
    }

    fn write_aligned(&mut self, target: &mut [u8], data: &[u8], alignment: usize) -> usize {
        let start = self.position;
        self.write_at(target, self.position, data);
        self.position = (self.position + data.len()).next_multiple_of(alignment);
        start
    }

    fn write_aligned_string(&mut self, target: &mut [u8], data: &str) -> usize {
        let len = data.len() as u64;
        let start = self.write_aligned(target, &len.to_le_bytes(), Cpu::WORD_SIZE);
        self.write_aligned(target, data.as_bytes(), Cpu::WORD_SIZE);
        start
    }

    fn write_aligned_symbol(&mut self, target: &mut [u8], name: &str, plist: Word) -> usize {
        let name_address = self.write_aligned_string(target, name);
        let symbol_address =
            self.write_aligned(target, &name_address.to_le_bytes(), Cpu::WORD_SIZE);
        self.write_aligned(target, &(u64::from(plist)).to_le_bytes(), Cpu::WORD_SIZE);
        symbol_address
    }

    fn write_instructions_at(
        &mut self,
        target: &mut [u8],
        address: usize,
        instructions: &[Instruction],
    ) {
        for (idx, instruction) in instructions.iter().enumerate() {
            let (lo, hi) = instruction.encode();
            self.write_at(
                target,
                address + (idx * Cpu::INSTRUCTION_SIZE),
                &[lo.to_le_bytes(), hi.to_le_bytes()].concat(),
            )
        }
    }

    fn new(start: usize) -> Self {
        Self { position: start }
    }
}
struct FirmwareHelper {}
impl FirmwareHelper {
    const BOOTSTRAP_HOOK: usize = MemoryLayout::CPU_RESERVED_END;
    const BOOTSTRAP_ALLOC_HOOK: usize = 0x600;
    const BOOTSTRAP_TRAP_HOOK: usize = 0x800;
    const BOOTSTRAP_OBJECTS: usize = 0x3000;
    const BOOTSTRAP_STACK_POSITION: usize = 0x2000; // Grows backwards
    const BOOTSTRAP_SCRATCH_ALLOC: usize = 0x2000; // Grows forwards
    const VIDEO_INTERRUPT: usize = InterruptTableOffset::END_RESERVED_INTERRUPTS + 0x00;
    const CURSOR_POSITION: usize = 0x14000;
    const VIDEO_INTERRUPT_ROUTINE: usize = 0x12000;

    fn make_firmware() -> Vec<u8> {
        let mut firmware = vec![0; 0x20000];

        // Starting at 0x3000, place constant symbols.
        let mut cursor = Cursor::new(Self::BOOTSTRAP_OBJECTS);
        let nil_symbol = cursor.write_aligned_symbol(&mut firmware, "nil", Word::undefined());
        let t_symbol = cursor.write_aligned_symbol(&mut firmware, "t", Word::undefined());

        // At 0x0000, place root objects.
        let nil = Word::symbol(nil_symbol as u64);
        firmware
            .as_mut_slice()
            .write_word(MemoryLayout::NIL_ROOT, nil.into());

        let t = Word::symbol(t_symbol as u64);
        firmware
            .as_mut_slice()
            .write_word(MemoryLayout::T_ROOT, t.into());

        // Finalize objects
        let nil_plist = nil_symbol + (SymbolLayout::PLIST_OFFSET);
        firmware.as_mut_slice().write_word(nil_plist, nil.into());
        let t_plist = t_symbol + (SymbolLayout::PLIST_OFFSET);
        firmware.as_mut_slice().write_word(t_plist, nil.into());

        // Set Vectors.
        firmware
            .as_mut_slice()
            .write_word(MemoryLayout::RESET_VECTOR, Self::BOOTSTRAP_HOOK as u64);
        firmware.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
            Self::BOOTSTRAP_ALLOC_HOOK as u64,
        );
        firmware.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::ALLOC_VECTOR,
            Self::BOOTSTRAP_ALLOC_HOOK as u64,
        );
        firmware.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + InterruptTableOffset::TRAP_VECTOR,
            Self::BOOTSTRAP_TRAP_HOOK as u64,
        );
        firmware.as_mut_slice().write_word(
            MemoryLayout::INTERRUPT_TABLE + Self::VIDEO_INTERRUPT * Cpu::WORD_SIZE,
            Self::VIDEO_INTERRUPT_ROUTINE as u64,
        );

        // Bootstrap program:
        cursor.write_instructions_at(
            &mut firmware,
            Self::BOOTSTRAP_HOOK,
            &parse_asm! {
                MOV A Cpu::SP, Self::BOOTSTRAP_STACK_POSITION;
                MOV R 0, Word::fixnum(0);
                MOV A 0, R 0;
                MOV [Self::CURSOR_POSITION], A 0;
                MOV R 0, Word::char('H' as u64);
                MOV R 1, Word::fixnum(0x07);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('e' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('l' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('l' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('o' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char(',' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char(' ' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('n' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('o' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('w' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char(' ' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('w' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('i' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('t' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('h' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 0, Word::char('\n' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 1, Word::fixnum(0x27);
                MOV R 0, Word::char('V' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 1, Word::fixnum(0x43);
                MOV R 0, Word::char('G' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 1, Word::fixnum(0x10);
                MOV R 0, Word::char('A' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                MOV R 1, Word::fixnum(0x85);
                MOV R 0, Word::char('!' as u64);
                INT Self::VIDEO_INTERRUPT as u64;
                HALT;
            }
            .as_slice(),
        );

        // Video interrupts
        cursor.write_instructions_at(
            &mut firmware,
            Self::VIDEO_INTERRUPT_ROUTINE,
            // A0 contains the specific interrupt. Only print_char is handled for now...
            // R0 contains the character to print as a char
            // R1 contains the attributes to save.
            &parse_asm! {
                PUSH A 0;
                PUSH A 1;
                PUSH A 2;
                PUSH R 0;
                PUSH R 1;
                PUSH R 2;
                PUSH R 3;
                PUSH R 4;
                PUSH R 5;
                // Read cursor position.
                MOV A 1, Self::CURSOR_POSITION;
                MOV R 2, [A 1];

                // Find Row(r4), Col(r3).
                DIV R 4, R 3, R 2, Word::fixnum(80);

                // If c == '\n', row++, col=0
                EQ R 5, R 0, Word::char('\n' as u64);
                JUMPIF R 5, [A Cpu::PC => 11 * Cpu::INSTRUCTION_SIZE as i64]; // GOTO: newline.

                // Else, print character and advance cursor.
                // 0xb8000 + cursorpos(r2) = attrib
                // 0xb8000 + cursorpos(r2) + 1 = char
                MUL R 2, R 2, Word::fixnum(2);
                GETPAYLOAD A 0, R 2;
                ADD A 0, A 0, 0xB8000; // VGA Start
                // Save ATTR
                GETPAYLOAD A 2, R 1;
                MOV8 [A 0 ], A 2;
                // Save CHAR
                GETPAYLOAD A 2, R 0;
                MOV8 [A 0 => 1], A 2;
                // advance COLUMN
                ADD R 3, R 3, Word::fixnum(1);

                // if col>=80, newline.
                GTE R 5, R 3, Word::fixnum(80);
                JUMPIFNOT R 5, [A Cpu::PC => 3 * Cpu::INSTRUCTION_SIZE as i64];
                // Newline
                ADD R 4, R 4, Word::fixnum(1);
                MOV R 3, Word::fixnum(0);

                // If row == 25, scroll (row--)
                // TODO, will HALT instead.
                GTE R 5, R 4, Word::fixnum(25);
                JUMPIFNOT R 5, [A Cpu::PC => 2 * Cpu::INSTRUCTION_SIZE as i64];
                HALT;

                // Save cursor position.
                MUL R 4, R 4, Word::fixnum(80);
                ADD R 2, R 4, R 3;
                MOV [A 1], R 2;


                POP R 5;
                POP R 4;
                POP R 3;
                POP R 2;
                POP R 1;
                POP R 0;
                POP A 2;
                POP A 1;
                POP A 0;
                IRETURN;
            }
            .as_slice(),
        );

        // Default allocator:
        let free_ptr = 0x3fff;
        firmware
            .as_mut_slice()
            .write_word(free_ptr, Self::BOOTSTRAP_SCRATCH_ALLOC as u64);
        cursor.write_instructions_at(
            &mut firmware,
            Self::BOOTSTRAP_ALLOC_HOOK,
            // Equivalent to:
            // A0 = *freeptr;
            // *freeptr += len;
            &parse_asm! {
                PUSH A 1;
                PUSH A 2;
                PUSH A 3;
                PUSH R 1;
                MOV A 1, free_ptr;
                MOV A 0, [A 1];
                GETPAYLOAD A 2, R 1;
                ADD A 3, A 0, A 2;
                MOV [A 1], A 3;
                POP R 1;
                POP A 3;
                POP A 2;
                POP A 1;
                IRETURN;
            }
            .as_slice(),
        );

        // Default trap
        cursor.write_instructions_at(
            &mut firmware,
            Self::BOOTSTRAP_TRAP_HOOK,
            &parse_asm! {
                HALT;
            }
            .as_slice(),
        );

        firmware
    }
}

pub struct WispMachine<'a> {
    cpu: Cpu,

    ram: &'a mut [u8],

    pub halted: bool,
}

impl<'a> WispMachine<'a> {
    pub fn reset(&mut self) {
        self.cpu.reset(self.ram);
    }

    pub fn step(&mut self) {
        self.cpu.full_step(&mut self.ram);
        if self.cpu.halted {
            self.halted = true;
        }
    }

    pub fn get_vga_ram(&self) -> &[u8] {
        &self.ram[0xa0000..0xc0000]
    }

    pub fn new(cpu: Cpu, ram: &'a mut [u8]) -> Self {
        let machine = Self {
            cpu,
            ram,
            halted: false,
        };

        let firmware = FirmwareHelper::make_firmware();
        machine.ram[0..firmware.len()].copy_from_slice(&firmware[..]);
        machine
    }
}
