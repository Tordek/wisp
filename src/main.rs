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

use sdl2::video::GLProfile;
use std::ffi::CString;
use std::ptr;
use std::time::Instant;

const FRAGMENT_SHADER_SRC: &str = include_str!("crt.glsl");
const VERTEX_SHADER_SRC: &str = include_str!("vertex.glsl");
static FONT: &[u8; 4096] = include_bytes!("cp437-8x16.bin");

// Size of texture holding the screen (includes overscan).
const DEAD_V: usize = 8;
const DEAD_H: usize = 14;
const TEX_W: usize = 720 + DEAD_H * 2;
const TEX_H: usize = 400 + DEAD_V * 2;

// Screen plane.
const BEZEL: f32 = 0.98;
const VERTICES: [f32; 16] = [
    // PosX,  PosY,  TexU,  TexV
    -1.0 * BEZEL,
    1.0 * BEZEL,
    0.0,
    0.0, // Top-Left
    1.0 * BEZEL,
    1.0 * BEZEL,
    1.0,
    0.0, // Top-Right
    1.0 * BEZEL,
    -1.0 * BEZEL,
    1.0,
    1.0, // Bottom-Right
    -1.0 * BEZEL,
    -1.0 * BEZEL,
    0.0,
    1.0, // Bottom-Left
];

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

    let program = unsafe {
        let vertex_shader = compile_shader(VERTEX_SHADER_SRC, gl::VERTEX_SHADER)?;
        let fragment_shader = compile_shader(FRAGMENT_SHADER_SRC, gl::FRAGMENT_SHADER)?;
        let result = link_program(vertex_shader, fragment_shader)?;
        gl::DeleteShader(vertex_shader);
        gl::DeleteShader(fragment_shader);
        result
    };

    let mut vao = 0;
    let mut vbo = 0;
    let mut t = 0;

    unsafe {
        gl::GenVertexArrays(1, &mut vao);
        gl::BindVertexArray(vao);

        gl::GenBuffers(1, &mut vbo);
        gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
        gl::BufferData(
            gl::ARRAY_BUFFER,
            (VERTICES.len() * std::mem::size_of::<f32>()) as isize,
            VERTICES.as_ptr() as *const _,
            gl::STATIC_DRAW,
        );

        let position =
            gl::GetAttribLocation(program, CString::new("position").unwrap().as_ptr()) as u32;
        let texcoord =
            gl::GetAttribLocation(program, CString::new("texcoord").unwrap().as_ptr()) as u32;

        gl::EnableVertexAttribArray(position);
        gl::VertexAttribPointer(
            position,
            2,
            gl::FLOAT,
            gl::FALSE,
            (4 * std::mem::size_of::<f32>()) as i32,
            ptr::null(),
        );

        gl::EnableVertexAttribArray(texcoord);
        gl::VertexAttribPointer(
            texcoord,
            2,
            gl::FLOAT,
            gl::FALSE,
            (4 * std::mem::size_of::<f32>()) as i32,
            (2 * std::mem::size_of::<f32>()) as *const _,
        );

        gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        gl::BindVertexArray(0);

        gl::UseProgram(program);

        // let tex_uniform = gl::GetUniformLocation(program, CString::new("tex").unwrap().as_ptr());
        t = gl::GetUniformLocation(program, CString::new("t").unwrap().as_ptr());

        // gl::Uniform1i(tex_uniform, 0);

        gl::UseProgram(0);
    }

    let mut vga_framebuffer = vec![0u8; TEX_W * TEX_H * 3];

    let mut vga_texture: gl::types::GLuint = 0;
    unsafe {
        gl::GenTextures(1, &mut vga_texture);
        gl::BindTexture(gl::TEXTURE_2D, vga_texture);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as i32);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as i32);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGB as i32,
            TEX_W as i32,
            TEX_H as i32,
            0,
            gl::RGB,
            gl::UNSIGNED_BYTE,
            ptr::null(),
        );
    }

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
        vga_framebuffer.fill(0);

        draw_text(
            &mut vga_framebuffer,
            DEAD_H,
            DEAD_V,
            "The quick brown fox jumps over the lazy dog",
            255,
            255,
            255,
        );

        // --- STEP B: GPU UPDATE ---
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(program);

            gl::ActiveTexture(gl::TEXTURE0);

            gl::BindTexture(gl::TEXTURE_2D, vga_texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as i32,
                TEX_W as i32,
                TEX_H as i32,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                ptr::null(),
            );

            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                DEAD_H as i32,
                DEAD_V as i32,
                720 as i32,
                400 as i32,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                vga_framebuffer.as_ptr() as *const _,
            );
            let elapsed_millis = start.elapsed().as_millis();

            gl::Uniform1f(t, elapsed_millis as f32 / 1000.0);

            gl::BindVertexArray(vao);
            gl::DrawArrays(gl::TRIANGLE_FAN, 0, 4);

            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }

        window.gl_swap_window();
    }

    Ok(())
}

// ---------------------------------------------------------
// Compile shaders
// ---------------------------------------------------------

unsafe fn compile_shader(src: &str, shader_type: gl::types::GLenum) -> Result<u32, String> {
    unsafe {
        let shader = gl::CreateShader(shader_type);

        let c_str = CString::new(src).unwrap();
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), ptr::null());
        gl::CompileShader(shader);

        let mut success = 0;
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut success);

        if success == 0 {
            let mut len = 0;
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);

            let mut buffer = vec![0u8; len as usize];
            gl::GetShaderInfoLog(shader, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut _);

            return Err(String::from_utf8_lossy(&buffer).into_owned());
        }
        Ok(shader)
    }
}

unsafe fn link_program(vertex_shader: u32, fragment_shader: u32) -> Result<u32, String> {
    unsafe {
        let program = gl::CreateProgram();

        gl::AttachShader(program, vertex_shader);
        gl::AttachShader(program, fragment_shader);
        gl::LinkProgram(program);

        let mut success = 0;
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut success);

        if success == 0 {
            let mut len = 0;
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);

            let mut buffer = vec![0u8; len as usize];
            gl::GetProgramInfoLog(program, len, ptr::null_mut(), buffer.as_mut_ptr() as *mut _);

            return Err(String::from_utf8_lossy(&buffer).into_owned());
        }

        gl::DetachShader(program, vertex_shader);
        gl::DetachShader(program, fragment_shader);

        Ok(program)
    }
}

fn draw_char(fb: &mut [u8], x: usize, y: usize, c: char, r: u8, g: u8, b: u8) {
    let glyph = &FONT[(c as usize) * 16..][..16];
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            if bits & (1 << (7 - col)) != 0 {
                put_pixel(fb, x + col, y + row, r, g, b);
            }
        }
    }
}
fn draw_text(fb: &mut [u8], x: usize, y: usize, text: &str, r: u8, g: u8, b: u8) {
    let mut cx = x;

    for ch in text.chars() {
        draw_char(fb, cx, y, ch, r, g, b);
        cx += 8;
    }
}

fn put_pixel(fb: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8) {
    if x >= 720 || y >= 400 {
        return;
    }

    let i = (y * 720 + x) * 3;
    fb[i] = r;
    fb[i + 1] = g;
    fb[i + 2] = b;
}
