use ogl33::*;
use std::{ffi::CString, collections::HashMap};
use nalgebra::{Vector4, Matrix4, Vector2, Vector3};
use std::mem::size_of;

use image::io::Reader as ImageReader;

pub mod simple;
pub mod textured;
pub mod text;
pub mod map;

#[derive(Copy, Clone)]
pub enum Attribute {
    Position = 0,
    Texture = 1,
    Normal = 2
}

pub fn attribute_name(attrib: Attribute) -> &'static str {
    match attrib {
        Attribute::Position => "position",
        Attribute::Texture => "texture",
        Attribute::Normal => "normal"
    }
}

pub enum VertexRange {
    Full,
    Range {
        first: i32,
        count: i32
    }
}

pub enum ShaderType {
    Vertex,
    Fragment,
    Geometry
}

struct CharPtrHolder {
    strs: Vec<CString>
}

impl CharPtrHolder {
    fn new() -> CharPtrHolder {
        CharPtrHolder { strs: Vec::<CString>::new() }
    }
}

trait ToCharPtr {
    fn to_char_ptr(&self, holder: &mut CharPtrHolder) -> *const i8;
}

impl ToCharPtr for str {
    fn to_char_ptr(&self, holder: &mut CharPtrHolder) -> *const i8 {
        let cstr = CString::new(self).unwrap();
        holder.strs.push(cstr);
        holder.strs.last().unwrap().as_ptr() as *const i8
    }
}

pub trait ToVec<T> {
    fn to_vec(&self) -> Vec<T>;
}

impl<T, U: ToVec<T>> ToVec<T> for Vec<U> {
    fn to_vec(self: &Vec<U>) -> Vec<T> {
        self.iter().fold(vec![], |mut v, m| {v.extend(m.to_vec()); v})
    }
}

pub struct Texture {
    handle: GLuint,
    owning: bool,
}

impl Texture {
    fn new(handle: GLuint, owning: bool) -> Texture {
        Texture {
            handle,
            owning
        }
    }

    // fine if I never end up using more than GL_TEXTURE0
    fn bind(&self) {
        self.bind_to(0);
    }

    fn bind_to(&self, location: GLuint) {
        unsafe {
            glActiveTexture(GL_TEXTURE0 + location);
            glBindTexture(GL_TEXTURE_2D, self.handle);
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        if self.owning {
            unsafe {
                glDeleteTextures(1, &self.handle);
            }
        }
    }
}

pub struct TextureLibrary {
    textures: HashMap<String, GLuint>,
    count: i32,
}

#[derive(PartialEq, Eq, Debug)]
pub enum TextureOptions {
    Red,
    Rgba,
    Repeating,
    Bilinear
}

pub fn make_texture(width: i32, height: i32, pixels: &Vec<u8>) -> Texture {
    assert!(pixels.len() == (width * height * 4) as usize);
    make_texture_impl(width, height, pixels.as_ptr() as *const c_void, true, &[])
}

// makes an rgba texture
fn make_texture_impl(width: i32, height: i32, pixels: *const c_void, owning: bool, options: &[TextureOptions]) -> Texture {
    Texture::new(unsafe {
        let mut handle: GLuint = 0;
        let format = {
            if options.iter().any(|op| *op == TextureOptions::Red) {
                GL_RED
            } else {
                GL_RGBA
            }
        };
        let wrap_mode = {
            if options.iter().any(|op| *op == TextureOptions::Repeating) {
                GL_REPEAT
            } else {
                GL_CLAMP_TO_EDGE
            }
        };
        let filter = {
            if options.iter().any(|op| *op == TextureOptions::Bilinear) {
                GL_LINEAR
            } else {
                GL_NEAREST
            }
        };
        glGenTextures(1, &mut handle);
        glBindTexture(GL_TEXTURE_2D, handle);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, wrap_mode as GLint);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, wrap_mode as GLint);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, filter as GLint);
        glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, filter as GLint);
        glTexImage2D(
            GL_TEXTURE_2D,
            0,
            format as GLint,
            width,
            height,
            0,
            format,
            GL_UNSIGNED_BYTE,
            pixels
        );
        glGenerateMipmap(GL_TEXTURE_2D);
        handle
    }, owning)
}

impl TextureLibrary {
    pub fn new() -> TextureLibrary {
        TextureLibrary{
            textures: HashMap::new(),
            count: 0
        }
    }

    pub fn make_texture(&mut self, source: &str, options: &[TextureOptions]) -> Texture {
        match self.textures.get(source) {
            Some(&handle) => Texture::new(handle, false),
            None => {
                let img_obj = ImageReader::open(source).unwrap().decode().unwrap();
                let img = img_obj.as_rgba8().unwrap();
                let img_data = img.as_raw();
                let texture = make_texture_impl(
                    img.width() as i32,
                    img.height() as i32,
                    img_data.as_ptr() as *const c_void,
                    false,
                    options);
                self.textures.insert(source.to_string(), texture.handle);
                self.count += 1;
                texture
            }
        }
    }

    pub fn make_texture_from(&mut self, width: u32, height: u32, data: &Vec<u8>, options: &[TextureOptions]) -> Texture {
        let name = format!("width: {}, height: {}, data.len(): {}, options: {:?}, num: {}", width, height, data.len(), options, self.count);
        let texture = make_texture_impl(width as i32, height as i32, data.as_ptr() as *const c_void, false, options);
        self.textures.insert(name, texture.handle);
        self.count += 1;
        texture
    }
}

impl Default for TextureLibrary {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for TextureLibrary {
    fn drop(&mut self) {
        self.textures.iter().for_each(|(_, handle)| {
            unsafe {
                glDeleteTextures(1, handle);
            }
        });
    }
}


// gets a uniform location for a string
// use this so you don't have to worry about figuring out CStrings
pub fn get_uniform(program: GLuint, uniform_name: &str) -> GLint {
    let mut holder = CharPtrHolder::new();
    unsafe {
        let loc = glGetUniformLocation(program, uniform_name.to_char_ptr(&mut holder));
        
        let err = glGetError();
        if err != 0 {
            panic!("Error after compiling shader: {}", err);
        }

        loc
    }
}

struct VAO {
    handle: GLuint,
    vertex_count: usize,
    buffers_to_delete: Vec<GLuint>
}

impl VAO {
    pub fn new<T: ToVec<f32>>(vertices: &T, desc: Vec<(Attribute, usize)>, usage: GLenum) -> VAO {
        let desc_length = desc.iter().fold(0, |s, n| s + n.1);
        unsafe {
            let mut vao = 0;
            let mut vbo = 0;
            glGenVertexArrays(1, &mut vao);
            glGenBuffers(1, &mut vbo);

            glBindVertexArray(vao);
            glBindBuffer(GL_ARRAY_BUFFER, vbo);

            let data = vertices.to_vec();
            glBufferData(
                GL_ARRAY_BUFFER,
                (data.len() * size_of::<f32>()) as isize,
                data.as_ptr().cast(),
                usage,
            );
            
            enable_vertex_attrib(desc);

            //glDeleteBuffers(1, &vbo); // since the vao has this buffer, it shouldn't get erased until the vao does
            // turns out that was false, we have to keep the buffers until deletion
            let buffers = vec![vbo];

            VAO { handle: vao, vertex_count: data.len() / desc_length, buffers_to_delete: buffers }
        }
    }

    pub fn render(&self, range: VertexRange) {
        let (first, count) = match range {
            VertexRange::Full => (0, self.vertex_count as i32),
            VertexRange::Range{first, count} => (first, count)
        };
        if first + count > self.vertex_count as i32 {
            panic!("Invalid vertex range for {} vertices: {}, {}", self.vertex_count, first, count);
        }
        unsafe {
            glBindVertexArray(self.handle);
            glDrawArrays(GL_TRIANGLES, first, count);
        }
    }
}

impl Drop for VAO {
    fn drop(&mut self) {
        unsafe {
            glDeleteVertexArrays(1, &self.handle);
            for buffer in &self.buffers_to_delete {
                glDeleteBuffers(1, buffer);
            }
        }
    }
}

unsafe fn enable_vertex_attrib(desc: Vec<(Attribute, usize)>) {
    let mut cu = 0;
    let mut sum: usize = 0;
    for &d in desc.iter() {
        sum += d.1;
    }
    for d in desc {
        glEnableVertexAttribArray(d.0 as GLuint);
        glVertexAttribPointer(
            d.0 as GLuint,
            d.1 as i32, 
            GL_FLOAT, 
            GL_FALSE, 
            (sum * size_of::<f32>()) as GLsizei, 
            (cu * size_of::<f32>()) as *const _);
        cu += d.1 as usize;
    }
}

pub fn shader_program(shaders: &Vec<GLuint>, attributes: &Vec<Attribute>) -> GLuint {
    unsafe {
        let shader_program = glCreateProgram();
        for shader in shaders {
            glAttachShader(shader_program, *shader);
        }
        for attribute in attributes {
            glBindAttribLocation(shader_program, *attribute as u32, attribute_name(*attribute).as_ptr() as *const i8);
        }
        glLinkProgram(shader_program);
        let mut success = 0;
        glGetProgramiv(shader_program, GL_LINK_STATUS, &mut success);
        if success == 0 {
            let mut v: Vec<u8> = Vec::with_capacity(1024);
            let mut log_len = 0_i32;
            glGetProgramInfoLog(shader_program, 1024, &mut log_len, v.as_mut_ptr().cast());
            v.set_len(log_len.try_into().unwrap());
            panic!("Program Link Error: {}", String::from_utf8_lossy(&v));
        }
        glUseProgram(shader_program);
        for &shader in shaders.iter() {
            glDeleteShader(shader); // since the shader is bound, it will be held onto until shader_program is deleted
        }

        shader_program
    }
}

pub fn make_shader(source: &str, shader_type: GLenum) -> GLuint {
    unsafe {
        let vertex_shader = glCreateShader(shader_type);
        glShaderSource(
            vertex_shader, 1,
            &(source.as_bytes().as_ptr().cast()),
            &(source.len().try_into().unwrap())
        );
        glCompileShader(vertex_shader);
        let mut success = 0;
        glGetShaderiv(vertex_shader, GL_COMPILE_STATUS, &mut success);
        if success == 0 {
            let mut v: Vec<u8> = Vec::with_capacity(1024);
            let mut log_len = 0_i32;
            glGetShaderInfoLog(
                vertex_shader, 1024, &mut log_len, 
                v.as_mut_ptr().cast());
            v.set_len(log_len.try_into().unwrap());
            panic!("Vertex Compile Error: {}", String::from_utf8_lossy(&v));
        } else {
            vertex_shader
        }
    }
}

pub fn make_matrix(position: Vector2<f32>, scale: Vector2<f32>, rotation: f32) -> Matrix4<f32> {
    Matrix4::<f32>::identity()
        * Matrix4::new_translation(&Vector3::new(position.x, position.y, 0.0))
        * Matrix4::from_euler_angles(rotation, 0.0, 0.0)
        * Matrix4::new_nonuniform_scaling(&Vector3::new(scale.x, scale.y, 0.0))
}
