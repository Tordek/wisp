use std::error::Error;

use crate::{
    bus::{Bus, Device, Native},
    cpu::{self, assembler},
    ram::RAM,
};

const BIOS_ASM: &str = include_str!("bios.asm");

#[allow(dead_code)]
#[derive(Debug)]
enum FirmwareError<'a> {
    AssemblerError(assembler::AssemblerError<'a>),
}

impl<'a> From<assembler::AssemblerError<'a>> for FirmwareError<'a> {
    fn from(value: assembler::AssemblerError<'a>) -> Self {
        FirmwareError::AssemblerError(value)
    }
}

impl<'a> std::fmt::Display for FirmwareError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Error while generating firmware.",)
    }
}

impl<'a> Error for FirmwareError<'a> {}

#[derive(Clone)]
pub struct Firmware {
    base: usize,
    rom: Vec<assembler::Section>,
}
impl Firmware {
    pub const KEYBOARD_INTERRUPT: usize =
        (cpu::InterruptTableOffset::END_RESERVED_INTERRUPTS.0 + 0x01) as usize;
    pub const VRAM_LOCATION: std::ops::Range<u64> = 0x00ffffff000b8000..0x00ffffff000c0000;
    pub const KEYBOARD_LOCATION: std::ops::Range<u64> = 0x00ffffff000c0000..0x00ffffff000d0000;

    fn make_firmware<'a>() -> Result<Firmware, FirmwareError<'a>> {
        let rom = assembler::assemble(BIOS_ASM)?;

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
            if section.base <= address && ((address - section.data.len()) < section.base) {
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
    cpu: cpu::Cpu,

    pub bus: Bus<'a>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum WispMachineError {
    SetupError(String),
}

impl<'a> WispMachine<'a> {
    pub fn reset(&mut self) {
        self.cpu.reset();
    }

    pub fn step(&mut self) {
        self.cpu.full_step(&mut self.bus);
    }

    pub fn new() -> Result<Self, WispMachineError> {
        let mut bus = Bus::new();
        let cpu = cpu::Cpu::default();
        let ram = RAM::new(2 << 20);
        let vram = RAM::new(2 << 14);
        let kbram = RAM::new(2 << 10);

        let firmware = Firmware::make_firmware()
            .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;

        bus.install(0x00000000..0xffffffff, Box::new(ram.ram_device))
            .expect("install");
        bus.install(
            0x00ffffff00000000..0x00ffffff00001000,
            Box::new(ram.configuration_device),
        )
        .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;

        bus.install(Firmware::VRAM_LOCATION, Box::new(vram.ram_device))
            .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;

        bus.install(Firmware::KEYBOARD_LOCATION, Box::new(kbram.ram_device))
            .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;

        bus.install(
            0x00fffffffff00000..0x00ffffffffffff00,
            Box::new(firmware.clone()),
        )
        .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;
        bus.install(0xffffffffffffff00..0xffffffffffffffff, Box::new(firmware))
            .map_err(|x| WispMachineError::SetupError(format!("{:?}", x)))?;

        Ok(Self { cpu, bus })
    }

    pub fn interrupt(&mut self, int_id: u64) {
        self.cpu.interrupt(int_id);
    }
}
