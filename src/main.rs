extern crate glfw;

use std::{ffi::CStr};
use glfw::{Action, Context, Key};
use nalgebra::{Matrix4, Vector4, Similarity2, Translation2, Rotation2, Scale, Scale2, Scale1, Matrix4x3, Matrix3x4, Similarity3, Translation3, Rotation3, Vector3, Vector2};
use ogl33::*;

use crate::graphics::*;

use image::io::Reader as ImageReader;

pub mod graphics;

mod game {
    use nalgebra::{Vector2, Matrix4, Orthographic3};
    use ogl33::glViewport;

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
    let mut context = graphics::Context::new();
    let mut render = graphics::textured::square_renderer(&mut context);

    let mut view = Matrix4::<f32>::identity();

    let texture = context.make_texture("tree.png");

    unsafe {
        glClearColor(0.0, 0.0, 0.0, 1.0);
        glEnable(GL_BLEND);
        glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
    }
    while !window.should_close() {
        unsafe {
            glClear(GL_COLOR_BUFFER_BIT);
        }

        let sim = Similarity3::<f32>::new(
            Vector3::new(100.0, 100.0, 0.0),
            Vector3::z() * std::f32::consts::FRAC_PI_4,
            100.0
        );
        context.matrix = game.ortho.as_matrix() * sim.to_homogeneous();
        context.color = Vector4::new(1.0, 1.0, 1.0, 1.0);
        context.range = graphics::VertexRange::Full;
        texture.bind();
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
