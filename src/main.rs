mod bus;
mod cpu;
mod gpu;
mod machine;
mod ram;

use std::time::Instant;

use sdl2::video::GLProfile;

use crate::machine::Firmware;

fn main() -> Result<(), String> {
    let mem = vec![0; 2 << 24];
    let mut machine = machine::WispMachine::new(cpu::Cpu::default(), mem);

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

    let gpu = gpu::Gpu::new().map_err(|_| "")?;

    let start = Instant::now();
    let mut event_pump = sdl_context.event_pump()?;

    let mem = vec![0; 2 << 24];
    let mut machine =
        machine::WispMachine::new(cpu::Cpu::default(), mem).map_err(|_| "Error making machine")?;

    machine.reset();

    'running: loop {
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => {
                    break 'running;
                }
                sdl2::event::Event::KeyDown {
                    timestamp: _,
                    window_id: _,
                    keycode,
                    scancode: _,
                    keymod: _,
                    repeat: _,
                } => {
                    machine.interrupt(Firmware::KEYBOARD_INTERRUPT as u64);
                    let key = keycode.unwrap().into_i32() as u8;
                    machine
                        .bus
                        .write_byte(bus::Address(Firmware::PRESSED_KEY_ID as u64), key)
                }

                _ => (),
            }
        }

        for _ in 1..1_000_000 {
            // Run 1 million cycles per draw.
            machine.step();
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
            gpu.render(&mut machine.bus, elapsed_millis as u32);
        }
        window.gl_swap_window();
    }

    Ok(())
}
