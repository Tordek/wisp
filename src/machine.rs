use std::collections::HashMap;

use crate::{
    cpu::{
        Cpu, Instruction, InterruptTableOffset, LispWord, MemoryLayout, Native,
        assembler::{assemble, layout, parse, resolve},
    },
    memory::Memory,
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
        let start = self.write_aligned(target, &len.to_le_bytes(), Cpu::WORD_SIZE as usize);
        self.write_aligned(target, data.as_bytes(), Cpu::WORD_SIZE as usize);
        start
    }

    fn write_aligned_symbol(&mut self, target: &mut [u8], name: &str, plist: LispWord) -> usize {
        let name_address = self.write_aligned_string(target, name);
        let symbol_address =
            self.write_aligned(target, &name_address.to_le_bytes(), Cpu::WORD_SIZE as usize);
        self.write_aligned(
            target,
            &(u64::from(Native::from(plist))).to_le_bytes(),
            Cpu::WORD_SIZE as usize,
        );
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
                address + (idx * Cpu::INSTRUCTION_SIZE as usize),
                &[lo.to_le_bytes(), hi.to_le_bytes()].concat(),
            )
        }
    }

    fn new(start: usize) -> Self {
        Self { position: start }
    }
}
pub struct FirmwareHelper {}
impl FirmwareHelper {
    pub const BOOTSTRAP_HOOK: usize = MemoryLayout::CPU_RESERVED_END.0 as usize;
    pub const BOOTSTRAP_ALLOC_HOOK: usize = 0x600;
    pub const BOOTSTRAP_TRAP_HOOK: usize = 0x800;
    pub const BOOTSTRAP_OBJECTS: usize = 0x3000;
    pub const BOOTSTRAP_STACK_POSITION: usize = 0x2000; // Grows backwards
    pub const BOOTSTRAP_SCRATCH_ALLOC: usize = 0x2000; // Grows forwards
    pub const VIDEO_INTERRUPT: usize =
        (InterruptTableOffset::END_RESERVED_INTERRUPTS.0 + 0x00) as usize;
    pub const KEYBOARD_INTERRUPT: usize =
        (InterruptTableOffset::END_RESERVED_INTERRUPTS.0 + 0x01) as usize;
    pub const CURSOR_POSITION: usize = 0x18000;
    pub const VIDEO_INTERRUPT_ROUTINE: usize = 0x12000;
    pub const KEYBOARD_INTERRUPT_ROUTINE: usize = 0x13000;
    pub const PRESSED_KEY_ID: usize = 0x14001;

    fn make_firmware() -> Vec<u8> {
        let mut symbols = HashMap::<&str, usize>::new();

        symbols.insert("bootstrap_objects", 0x3000);

        let parsed_asm = parse(stringify! {
            .ord 0
            nil: .symbol 'nil_symbol,
            t: .symbol 't_symbol,

            .ord 'bootstrap_objects
            nil_str: .str "nil"
            t_str: .str "t"
            nil_symbol:
                .w 'nil_str
                .w 'nil
            t_symbol:
                .w 't_str
                .w 'nil

            .ord 'reset_vector
            .w 'boostrap

            .ord 'interrupt_table
            .w alloc
            .w alloc
            .w trap

            .ord 'user_interrupt_table
            video_interrupt_hook_address: .w 'video_interrupt
            keyboard_interrupt_hook_address: .w 'keyboard_interrupt

        bootstrap:
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
            MOV R 0, Word::char('\n' as u64);
            INT Self::VIDEO_INTERRUPT as u64;
            // JUMP [A Cpu::PC];
            HALT;

        video_interrupt:
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
            NOP;

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

        keyboard_interrupt:
            PUSH A 0;
            PUSH R 0;
            PUSH R 1;
            MOV R 0, Word::fixnum(0x07);
            MOV A 0, [Self::PRESSED_KEY_ID];
            SETPAYLOAD R 0, A 0;
            MOV R 1, Word::fixnum(0x07);
            // MOV R 0, Word::char('o' as u64);
            INT Self::VIDEO_INTERRUPT as u64;
            POP R 1;
            POP R 0;
            POP A 0;
            IRETURN;

        alloc:
        // Equivalent to:
        // A0 = *freeptr;
        // *freeptr += len;
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

        _trap:
            HALT;
        })
        .unwrap();

        let locate = layout(&parsed_asm);
        let resolved = resolve(&parsed_asm, &locate).unwrap();
        let firmware = assemble(&resolved);

        firmware
    }
}

pub struct WispMachine {
    cpu: Cpu,

    pub ram: Memory,

    pub halted: bool,
}

impl<'a> WispMachine {
    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.ram);
    }

    pub fn step(&mut self) {
        self.cpu.full_step(&mut self.ram);
        if self.cpu.halted {
            self.halted = true;
        }
    }

    pub fn get_vga_ram(&self) -> &[u8] {
        &self.ram.bytes[0xa0000..0xc0000]
    }

    pub fn new(cpu: Cpu, ram: Vec<u8>) -> Self {
        let mut machine = Self {
            cpu,
            ram: Memory { bytes: ram },
            halted: false,
        };

        let firmware = FirmwareHelper::make_firmware();
        machine.ram.bytes[0..firmware.len()].copy_from_slice(&firmware[..]);
        machine
    }

    pub fn interrupt(&mut self, int_id: u64) {
        self.cpu.interrupt(int_id);
    }
}
