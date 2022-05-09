use nalgebra::{Vector2, Orthographic3};
use ogl33::glViewport;

use std::ffi::CStr;
use glfw::{Action, Context, Key};
use nalgebra::{Vector4, Vector3, Similarity3};
use ogl33::*;
use std::net::SocketAddr;

use crate::{graphics, chatbox, networking};

#[derive(Clone)]
pub enum State {
    DEFAULT,
    TYPING
}

pub struct Game<'a> {
    pub window_size: Vector2<i32>,
    pub ortho: Orthographic3<f32>,
    chatbox: chatbox::Chatbox<'a>,
    pub state: State,
    connection: networking::client::Connection,
}

impl Game<'_> {
    pub fn run(default_server: &SocketAddr) {
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
        window.set_char_polling(true);
        window.set_size_polling(true);
        window.make_current();

        unsafe {
            load_gl_with(|f_name| {
                let cstr = CStr::from_ptr(f_name);
                let str = cstr.to_str().expect("Failed to convert OGL function name");
                window.get_proc_address(&str)
            });
        }
        
        let mut font_library = graphics::text::FontLibrary::new();
        let text = font_library.make_font("arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));
        let simple_render = graphics::simple::Renderer::new_square();
        let mut game = Game {
            window_size: Vector2::<i32>::new(width, height),
            ortho: Orthographic3::<f32>::new(0.0, width as f32, height as f32, 0.0, 0.0, 1.0),
            chatbox: chatbox::Chatbox::new(&text, &simple_render, 7, 40, 800.0),
            state: State::DEFAULT,
            connection: networking::client::Connection::new(default_server).unwrap()
        };
        game.window_size(width, height);
        let render = graphics::textured::Renderer::new_square();

        // let mut texture_library = graphics::TextureLibrary::new();
        // let texture = texture_library.make_texture("tree.png");
        game.chatbox.println("Hello");

        let fontinfo = graphics::text::make_font(&font_library, "arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));
        let font_texture = graphics::make_texture(fontinfo.image_size.x as i32, fontinfo.image_size.y as i32, &graphics::text::convert_r_to_rgba(&fontinfo.image_buffer));

        unsafe {
            glClearColor(0.0, 0.0, 0.0, 1.0);
            glEnable(GL_BLEND);
            glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        }

        let mut last_time = glfw.get_time();
        while !window.should_close() {
            let current_time = glfw.get_time();
            let delta_time = (current_time - last_time) as f32;
            last_time = current_time;
            unsafe {
                glClear(GL_COLOR_BUFFER_BIT);
            }

            game.connection.flush(); // send messages
            let messages = game.connection.poll();
            for message in messages {
                game.chatbox.println(format!("Server: {}", String::from_utf8_lossy(&message)).as_str());
            }

            let sim = Similarity3::<f32>::new(
                Vector3::new(100.0, 500.0, 0.0),
                Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                1.0
            );
            let matrix = game.ortho.as_matrix() * sim.to_homogeneous();
            let msg = String::from("Hihfas \u{2122} dhofhoas dohfaho hoh7o  H&AH&*( (&*DF(&SD(&*F&*(SD^*(F(&^!)*#$^&$^!_$^)$&*)RUHR\"");
            let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
            text.render(&matrix, msg.as_str(), &color);

            let sim = Similarity3::<f32>::new(
                Vector3::new(400.0, 400.0, 0.0),
                Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                800.0
            );
            render.render(
                game.ortho.as_matrix() * sim.to_homogeneous(),
                Vector4::new(1.0, 1.0, 1.0, 1.0),
                &font_texture,
                graphics::VertexRange::Full
            );

            game.chatbox.render(game.ortho.as_matrix(), delta_time);

            window.swap_buffers();
            glfw.poll_events();
            for (_, event) in glfw::flush_messages(&events) {
                match (game.state.clone(), event) {
                    (_, glfw::WindowEvent::Key(Key::Escape, _, Action::Press, _)) => {
                        window.set_should_close(true);
                        game.state = State::DEFAULT;
                    },
                    (_, glfw::WindowEvent::Size(width, height)) => {
                        game.window_size(width, height);
                    },
                    (State::TYPING, glfw::WindowEvent::Char(char)) => {
                        game.chatbox.add_typing(char);
                    },
                    (State::TYPING, glfw::WindowEvent::Key(Key::Backspace, _, Action::Press, _)) |
                    (State::TYPING, glfw::WindowEvent::Key(Key::Backspace, _, Action::Repeat, _)) => {
                        game.chatbox.remove_typing(1);
                    },
                    (State::TYPING, glfw::WindowEvent::Key(Key::Enter, _, Action::Press, _)) => {
                        let line = game.chatbox.get_typing().clone();
                        if line.len() != 0 {
                            game.chatbox.erase_typing();
                            game.process_chat(line.as_str());
                        } else {
                            game.state = State::DEFAULT;
                            game.chatbox.set_typing_flicker(false);
                        }
                    },
                    (State::DEFAULT, glfw::WindowEvent::Key(Key::Enter, _, Action::Press, _)) => {
                        game.state = State::TYPING;
                        game.chatbox.set_typing_flicker(true);
                    },
                    (State::DEFAULT, glfw::WindowEvent::Key(Key::Slash, _, Action::Press, _)) => {
                        game.state = State::TYPING;
                        game.chatbox.set_typing_flicker(true);
                        game.chatbox.add_typing('/');
                    }
                    _ => {}
                }
            }
        }
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

    pub fn process_chat(&mut self, command: &str) {
        if !command.starts_with('/') {
            self.process_chat((String::from("/send ") + command).as_str())
        } else {
            let split: Vec<&str> = command[1..].split(' ').collect();
            match &split[..] {
                ["hello", "world"] => {
                    self.chatbox.println("Hello world!");
                },
                ["send", ..] => {
                    self.connection.send(command[1..].as_bytes().to_vec());
                },
                ["print", _, ..] => {
                    self.chatbox.println(&command[("/print ".len())..]);
                }
                ["server", address] => {
                    let address: SocketAddr = match address.parse() {
                        Err(e) => {
                            self.chatbox.println(format!("Error parsing address: {}", e).as_str());
                            return;
                        },
                        Ok(address) => address
                    };
                    self.connection.set_server_address(&address);
                    self.chatbox.println(format!("Successfully changed server address to {}", address.to_string()).as_str());
                }
                _ => self.chatbox.println("Failed to parse command.")
            }
        }
    }
}