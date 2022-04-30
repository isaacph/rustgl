extern crate glfw;

use std::{ffi::CStr};
use glfw::{Action, Context, Key};
use nalgebra::{Matrix4, Vector4, Similarity2, Translation2, Rotation2, Scale, Scale2, Scale1, Matrix4x3, Matrix3x4, Similarity3, Translation3, Rotation3, Vector3};
use ogl33::*;

mod game {
    use nalgebra::{Vector2, Matrix4, Orthographic3};
    use ogl33::glViewport;

    pub mod graphics {
        use ogl33::*;
        use std::{ffi::CString};
        use nalgebra::{Vector4, Matrix4, Matrix3, Matrix4x3, Matrix3x4};
        use std::mem::size_of;

        type RenderFunction = Box<dyn FnMut(&Context) -> ()>;

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
                        vaos: Vec::new()
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

        trait Homo4D<T> {
            fn to4d(&self) -> Matrix4<T>;
        }

        pub mod simple {
            use nalgebra::Vector2;

            use super::*;
            pub struct Vertex {
                position: Vector2<f32>
            }
        
            impl ToVec<f32> for Vertex {
                fn to_vec(&self) -> Vec<f32> {
                    vec![self.position.x, self.position.y]
                } 
            }

            impl Vertex {
                pub fn new(x: f32, y: f32) -> Vertex {
                    Vertex {
                        position: Vector2::<f32>::new(x, y)
                    }
                }
            }

            impl Clone for Vertex {
                fn clone(&self) -> Self {
                    Vertex {
                        position: self.position.clone()
                    }
                }
            }

            pub fn shader(context: &mut Context) -> RenderFunction {
                const VERT_SHADER: &str = r#"
                    #version 330
                    in vec2 pos;
                    uniform mat4 matrix;
                    void main() {
                        gl_Position = matrix * vec4(pos.x, pos.y, 0.0, 1.0);
                    }
                "#;
                const FRAG_SHADER: &str = r#"
                    #version 330
                    uniform vec4 color;
                    out vec4 final_color;
                    void main() {
                        final_color = color;
                    }
                "#;
                let shader_program = context.shader_program(
                    &vec![
                        context.make_shader(VERT_SHADER, GL_VERTEX_SHADER),
                        context.make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
                    &vec![Attribute::Position]);
                let u_color = context.get_uniform(shader_program, "color");
                let u_matrix = context.get_uniform(shader_program, "matrix");
                Box::new(move |context: &Context| {
                    unsafe {
                        glUseProgram(shader_program);
                        glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
                        glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
                    }
                })
            }

            pub fn renderer(context: &mut Context) -> RenderFunction {
                let vertices =
                        [Vertex::new(-0.5, -0.5),
                        Vertex::new(0.5, -0.5),
                        Vertex::new(0.5, 0.5),
                        Vertex::new(0.5, 0.5),
                        Vertex::new(-0.5, 0.5),
                        Vertex::new(-0.5, -0.5)];
                let mut render_vao = context.vao(
                    vertices.to_vec(),
                    vec![(Attribute::Position, 2)],
                    GL_STATIC_DRAW
                );
                let mut use_shader = shader(context);
                Box::new(move |context: &Context| {
                    use_shader(&context);
                    render_vao(&context);
                })
            }
        }

        struct PersistentObjects {
            programs: Vec<GLuint>,
            shaders: Vec<GLuint>,
            vbos: Vec<GLuint>,
            vaos: Vec<GLuint>
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

        impl Context {
            pub fn vao<T: ToVec<f32>>(self: &mut Context, vertices: T, desc: Vec<(Attribute, usize)>, usage: GLenum) -> RenderFunction {
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
        }
    }

    pub struct Game {
        pub window_size: Vector2<i32>,
        pub ortho: Orthographic3<f32>
    }

    impl Game {
        pub fn new(width: i32, height: i32) -> Game {
            let mut obj = Game {
                window_size: Vector2::<i32>::new(width, height),
                ortho: Orthographic3::<f32>::new(0.0, width as f32, height as f32, 0.0, 0.0, 1.0)
            };
            obj.window_size(width, height);
            obj
        }

        pub fn window_size(&mut self, width: i32, height: i32) {
            self.window_size.x = width;
            self.window_size.y = height;
            self.ortho.set_right(width as f32);
            self.ortho.set_bottom(height as f32);
            unsafe {
                glViewport(0, 0, width, height);
            }
        }
    }
}

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    let (width, height) = (800, 600);

    glfw.window_hint(glfw::WindowHint::ContextVersionMajor(3));
    glfw.window_hint(glfw::WindowHint::ContextVersionMinor(3));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));

    let (mut window, events) = 
        glfw.create_window(width as u32, height as u32, "Hello Window",
            glfw::WindowMode::Windowed)
            .expect("Failed to create GLFW window.");

    window.set_key_polling(true);
    window.set_size_polling(true);
    window.make_current();

    unsafe {
        load_gl_with(|f_name| {
            let cstr = CStr::from_ptr(f_name);
            let str = cstr.to_str().expect("Failed to convert OGL function name");
            window.get_proc_address(&str)
        });
    }
    
    let mut game = game::Game::new(width, height);

    let mut context = game::graphics::Context::new();
    let mut render = game::graphics::simple::renderer(&mut context);

    let mut view = Matrix4::<f32>::identity();

    unsafe {
        glClearColor(0.2, 0.3, 0.3, 1.0);
    }
    while !window.should_close() {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);
        }

        let axisangle = Vector3::z() * std::f32::consts::FRAC_PI_4;
        let sim = Similarity3::<f32>::new(
            Vector3::new(0.0, 0.0, 0.0),
            axisangle,
            1.5
        );
        let x = sim.to_homogeneous();
        context.matrix = x;
        context.color = Vector4::<f32>::new(1.0, 1.0, 1.0, 1.0);
        context.range = game::graphics::VertexRange::Full;
        render(&context);

        window.swap_buffers();
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    window.set_should_close(true)
                },
                glfw::WindowEvent::Size(width, height) => {
                    game.window_size(width, height);
                },
                _ => {}
            }
        }
    }
}
