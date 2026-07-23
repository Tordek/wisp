use std::ffi::CString;
use std::ptr;

const FRAGMENT_SHADER_SRC: &str = include_str!("crt.glsl.frag");
const VERTEX_SHADER_SRC: &str = include_str!("vertex.glsl.vert");
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

pub struct Gpu {
    program: u32,
    vga_texture: gl::types::GLuint,
    vao: u32,
    t: gl::types::GLint,
}
impl Gpu {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
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

        let t = unsafe {
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

            let t = gl::GetUniformLocation(program, CString::new("t").unwrap().as_ptr());

            gl::UseProgram(0);

            t
        };

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

        Ok(Self {
            program,
            t,
            vga_texture,
            vao,
        })
    }
    pub unsafe fn render(&self, vga_ram: &[u8], elapsed_millis: u32) {
        let mut vga_framebuffer = vec![0u8; 720 * 400 * 3];
        vga_framebuffer.fill(0);
        for i in 0..(25*80) {
            let position = 0x18000 + 2 * i;
            let attributes = vga_ram[position];
            let character = vga_ram[position + 1];

            draw_char(
                &mut vga_framebuffer,
                (i % 80) * 9,
                (i / 80) * 16,
                character,
                attributes,
                (elapsed_millis / 1000) & 1 == 1,
            )
        }
        unsafe {
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::UseProgram(self.program);

            gl::ActiveTexture(gl::TEXTURE0);

            gl::BindTexture(gl::TEXTURE_2D, self.vga_texture);
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

            gl::Uniform1f(self.t, elapsed_millis as f32 / 1000.0);

            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_FAN, 0, 4);

            gl::BindTexture(gl::TEXTURE_2D, 0);
            gl::BindVertexArray(0);
            gl::UseProgram(0);
        }
    }
}

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

fn draw_char(fb: &mut [u8], x: usize, y: usize, c: u8, attributes: u8, t: bool) {
    let glyph = &FONT[(c as usize) * 16..][..16];
    for (row, bits) in glyph.iter().enumerate() {
        for col in 0..8 {
            put_pixel(
                fb,
                x + col,
                y + row,
                bits & (1 << (7 - col)) != 0,
                t,
                attributes,
            );
        }
        put_pixel(fb, x + 8, y + row, bits & (1) != 0, t, attributes);
    }
}

fn put_pixel(fb: &mut [u8], x: usize, y: usize, on: bool, t: bool, attributes: u8) {
    if x >= 720 || y >= 400 {
        return;
    }

    let i = (y * 720 + x) * 3;
    let (r, g, b) = vga_color(on, t, attributes);
    fb[i] = r;
    fb[i + 1] = g;
    fb[i + 2] = b;
}

fn vga_color(on: bool, t: bool, color: u8) -> (u8, u8, u8) {
    let c = if on && !(color & 0x80 == 0x80 && t) {
        color & 0x0f
    } else {
        (color >> 4) & 0x07
    };
    match c {
        0 => (9,4,4),
        1 => (191, 63, 52),
        2 => (2, 125, 45),
        3 => (190, 157, 57),
        4 => (35, 111, 178),
        5 => (153, 88, 119),
        6 => (3, 121, 118),
        7 => (205, 205, 205),

        8 => (104, 97, 94),
        9 => (230, 78, 64),
        10 => (2, 152, 56),
        11 => (233, 190, 40),
        12 => (44, 135, 214),
        13 => (184, 107, 144),
        14 => (4, 146, 142),
        15 => (246, 246, 246),
        _ => (0, 0, 0),
    }
}
