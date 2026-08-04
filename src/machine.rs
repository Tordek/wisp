use std::{collections::HashMap, error::Error};

use crate::{
    bus::{Address, Bus, Device, Native},
    cpu::{
        Cpu, InterruptTableOffset, MemoryLayout,
        assembler::{AssemblerError, Section, assemble},
    },
    ram::Memory,
};

const BIOS_ASM: &str = include_str!("bios.asm");

#[derive(Debug)]
enum FirmwareError<'a> {
    AssemblerError(AssemblerError<'a>),
}

impl<'a> From<AssemblerError<'a>> for FirmwareError<'a> {
    fn from(value: AssemblerError<'a>) -> Self {
        FirmwareError::AssemblerError(value)
    }
}

pub struct Firmware {
    base: usize,
    rom: Vec<Section>,
}
impl Firmware {
    pub const KEYBOARD_INTERRUPT: usize =
        (InterruptTableOffset::END_RESERVED_INTERRUPTS.0 + 0x01) as usize;
    pub const PRESSED_KEY_ID: usize = 0x28008;

    fn make_firmware<'a>() -> Result<Firmware, FirmwareError<'a>> {
        let mut symbols = HashMap::<&str, usize>::new();

        symbols.insert("bootstrap_objects", 0x3000);

        let rom = assemble(BIOS_ASM)?;

        Ok(Firmware { base: 0, rom })
    }
}
impl Device for Firmware {
    fn set_location(&mut self, base_address: crate::bus::Address) {
        self.base = base_address.0 as usize
    }
    fn read_byte(&self, address: crate::bus::Address) -> u8 {
        let address = address.0 as usize + self.base;
        for section in &self.rom {
            if section.base <= address && ((address) < section.base + section.data.len()) {
                return section.data[address - section.base];
            }
        }
        0
    }

    fn read_word(&self, address: crate::bus::Address) -> crate::bus::Native {
        let address = address.0 as usize + self.base;
        for section in &self.rom {
            if section.base <= address as usize && ((address) < section.base + section.data.len()) {
                return Native(u64::from_le_bytes(
                    section.data[address - section.base..][..8]
                        .try_into()
                        .expect("Out of bands memory access."),
                ));
            }
        }
        Native(0)
    }

    // NOP: ROMs.
    fn write_byte(&mut self, _: crate::bus::Address, _: u8) {}
    fn write_word(&mut self, _: crate::bus::Address, _: crate::bus::Native) {}
}

pub struct WispMachine<'a> {
    cpu: Cpu,

    pub bus: Bus<'a>,

    pub halted: bool,
}

enum WispMachineError {
    SetupError,
}

impl<'a> WispMachine<'a> {
    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn step(&mut self) {
        self.cpu.full_step(&mut self.bus);
        if self.cpu.halted {
            self.halted = true;
        }
    }

    pub fn new(cpu: Cpu, ram: Vec<u8>) -> Result<Self, Box<dyn Error>> {
        let mut bus = Bus::new();
        let ram = Memory {
            base: Address(0),
            bytes: ram,
        };
        let firmware = Firmware::make_firmware().expect("compiled");

        bus.install(0x00000000..0xffffffff, Box::new(ram))
            .expect("install");

        bus.install(
            MemoryLayout::CPU_ROOT.0..MemoryLayout::CPU_ROOT_END.0,
            Box::new(firmware),
        )
        .expect("install");

        Ok(Self {
            cpu,
            bus,
            halted: false,
        })
    }

    pub fn interrupt(&mut self, int_id: u64) {
        self.cpu.interrupt(int_id);
    }
}
