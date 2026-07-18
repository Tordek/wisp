mod cpu;
mod machine;
mod memory;

use crate::{
    cpu::Cpu,
    machine::{WispMachine, WispMemory},
};

fn main() {
    let mut machine = WispMachine::new(Cpu::default(), WispMemory::new(2 << 24));

    machine.reset();

    while !machine.halted {
        machine.step();
    }
}
