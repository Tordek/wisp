use std::collections::HashMap;

use crate::{
    cpu::{Cpu, InterruptTableOffset, MemoryLayout, assembler::assemble},
    memory::Memory,
};

const BIOS_ASM: &str = include_str!("bios.asm");
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
    pub const PRESSED_KEY_ID: usize = 0x18008;

    fn make_firmware() -> Vec<u8> {
        let mut symbols = HashMap::<&str, usize>::new();

        symbols.insert("bootstrap_objects", 0x3000);

        assemble(BIOS_ASM).unwrap()
    }
}

pub struct WispMachine {
    cpu: Cpu,

    pub ram: Memory,

    pub halted: bool,
}

impl WispMachine {
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
