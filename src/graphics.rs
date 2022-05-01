use ogl33::*;
use std::{ffi::CString, collections::HashMap, hash::Hash};
use nalgebra::{Vector4, Matrix4, Matrix3, Matrix4x3, Matrix3x4};
use std::mem::size_of;

use image::io::Reader as ImageReader;

type RenderFunction = Box<dyn FnMut(&Context) -> ()>;

pub mod simple;
pub mod textured;

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

pub struct Context {
    pub range: VertexRange,
    pub matrix: Matrix4<f32>,
    pub color: Vector4<f32>,
    persistent_objects: PersistentObjects
}

impl Context {
    pub fn new() -> Context {
        Context{
            range: VertexRange::Full,
            matrix: Matrix4::identity(),
            color: Vector4::new(1.0, 1.0, 1.0, 1.0),
            persistent_objects: PersistentObjects {
                programs: Vec::new(),
                shaders: Vec::new(),
                vbos: Vec::new(),
                vaos: Vec::new(),
                textures: HashMap::new()
            }
        }
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

struct PersistentObjects {
    programs: Vec<GLuint>,
    shaders: Vec<GLuint>,
    vbos: Vec<GLuint>,
    vaos: Vec<GLuint>,
    textures: HashMap<String, GLuint>
}

impl Drop for PersistentObjects {
    fn drop(&mut self) {
        unsafe {
            for &p in self.programs.iter() {
                glDeleteProgram(p);
            }
            for &s in self.shaders.iter() {
                glDeleteShader(s);
            }
            for &v in self.vbos.iter() {
                glDeleteBuffers(1, &v);
            }
            for &v in self.vaos.iter() {
                glDeleteVertexArrays(1, &v);
            }
            for (_, &v) in self.textures.iter() {
                glDeleteTextures(1, &v);
            }
        }
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
    handle: GLuint
}

impl Texture {
    fn new(handle: GLuint) -> Texture {
        Texture {
            handle: handle
        }
    }

    // fine if I never end up using more than GL_TEXTURE0
    pub fn bind(&self) {
        self.bind_to(0);
    }

    pub fn bind_to(&self, location: GLuint) {
        unsafe {
            glActiveTexture(GL_TEXTURE0 + location);
            glBindTexture(GL_TEXTURE_2D, self.handle);
        }
    }
}

impl Context {
    pub fn vao<T: ToVec<f32>>(self: &mut Context, vertices: &T, desc: Vec<(Attribute, usize)>, usage: GLenum) -> RenderFunction {
        let desc_length = desc.iter().fold(0, |s, n| s + n.1);
        let (vao, vertex_count) =
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
            
            self.enable_vertex_attrib(desc);

            self.persistent_objects.vbos.push(vbo);
            self.persistent_objects.vaos.push(vao);

            (vao, data.len() / desc_length)
        };
        Box::new(move |context: &Context| {
            let (first, count) = match context.range {
                VertexRange::Full => (0, vertex_count as i32),
                VertexRange::Range{first, count} => (first, count)
            };
            unsafe {
                glBindVertexArray(vao);
                glDrawArrays(GL_TRIANGLES, first, count);
            }
        })
    }

    unsafe fn enable_vertex_attrib(&self, desc: Vec<(Attribute, usize)>) {
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

    // gets a uniform location for a string
    // use this so you don't have to worry about figuring out CStrings
    pub fn get_uniform(self: &mut Context, program: GLuint, uniform_name: &str) -> GLint {
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

    pub fn shader_program(self: &mut Context, shaders: &Vec<GLuint>, attributes: &Vec<Attribute>) -> GLuint {
        unsafe {
            let shader_program = glCreateProgram();
            for shader in shaders {
                glAttachShader(shader_program, shader.clone());
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
                self.persistent_objects.shaders.push(shader);
            }
            self.persistent_objects.programs.push(shader_program);

            shader_program
        }
    }

    pub fn make_shader(&self, source: &str, shader_type: GLenum) -> GLuint {
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

    pub fn make_texture(&mut self, source: &str) -> Texture {
        match self.persistent_objects.textures.get(source) {
            Some(&handle) => Texture::new(handle),
            None => Texture::new({
                let img_obj = ImageReader::open(source).unwrap().decode().unwrap();
                let img = img_obj.as_rgba8().unwrap();
                let handle = unsafe {
                    let mut handle: GLuint = 0;
                    let img_data = img.as_raw();
                    glGenTextures(1, &mut handle);
                    glBindTexture(GL_TEXTURE_2D, handle);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as GLint);
                    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as GLint);
                    glGenerateMipmap(GL_TEXTURE_2D);
                    glTexImage2D(
                        GL_TEXTURE_2D,
                        0,
                        GL_RGBA as GLint,
                        img.width() as GLsizei,
                        img.height() as GLsizei,
                        0,
                        GL_RGBA,
                        GL_UNSIGNED_BYTE,
                        img_data.as_ptr() as *const c_void
                    );
                    handle
                };
                self.persistent_objects.textures.insert(source.to_string(), handle);
                handle
            })
        }
    }
}
