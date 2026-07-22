// mod cpu;
// mod machine;
// mod memory;

// use crate::{
//     cpu::Cpu,
//     machine::{WispMachine, WispMemory},
// };

// fn main() {
//     let mut machine = WispMachine::new(Cpu::default(), WispMemory::new(2 << 24));

//     machine.reset();

//     while !machine.halted {
//         machine.step();
//     }
// }

mod gpu;

use std::time::Instant;

use sdl2::video::GLProfile;
use std::ffi::CString;

use crate::gpu::Gpu;

fn main() -> Result<(), String> {
    let sdl_context = sdl2::init()?;

    let video_subsystem = sdl_context.video()?;
    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(GLProfile::Core);
    gl_attr.set_context_version(3, 2);

    // Create the Window at proper 4:3 display scale
    let window = video_subsystem
        .window("Rust VGA Emulator + CRT Shader", 960, 720)
        .resizable()
        .opengl()
        .position_centered()
        .build()
        .map_err(|e| e.to_string())?;

    let _context = window.gl_create_context()?;
    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const _);

    let mut gpu = Gpu::new().map_err(|_| "")?;

    let start = Instant::now();
    let mut event_pump = sdl_context.event_pump()?;
    'running: loop {
        for event in event_pump.poll_iter() {
            if let sdl2::event::Event::Quit { .. } = event {
                break 'running;
            }
        }
        // --- STEP A: EMULATOR WRITE ---
        // Write text pixels out directly into your native 720x400 byte buffer
        // Example: Turn some pixels bright green or gray to draw your letters
        // for i in 0..1000 {
        //     vga_framebuffer[i * 3] = 0; // R
        //     vga_framebuffer[i * 3 + 1] = 255; // G (VGA Green)
        //     vga_framebuffer[i * 3 + 2] = 0; // B
        // }

        let elapsed_millis = start.elapsed().as_millis();
        unsafe {
            gpu.render(elapsed_millis as u32);
        }
        window.gl_swap_window();
    }

    Ok(())
}
