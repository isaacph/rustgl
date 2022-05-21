use nalgebra::{Vector2, Orthographic3};
use ogl33::glViewport;
use serde::{Serialize, Deserialize};

use std::ffi::CStr;
use glfw::{Action, Context, Key};
use nalgebra::{Vector4, Vector3, Similarity3};
use ogl33::*;
use std::net::SocketAddr;

use crate::{
    graphics,
    chatbox,
    networking::{
        self,
        server::ConnectionID},
    server::{
        Server,
        StopServer},
    networking_wrapping::{
        ClientCommand,
        ServerCommand,
        SendClientCommands,
        ExecuteClientCommands,
        SendServerCommands},
    world::{
        World,
        GenerateCharacter,
        character::{
            CharacterID,
            CharacterIDGenerator
        }
    },
};


#[derive(Serialize, Deserialize, Debug)]
pub struct EchoMessage {
    message: String
}

impl EchoMessage {
    pub fn new(message: String) -> Self {
        EchoMessage {
            message
        }
    }
}

impl<'a> ClientCommand<'a> for EchoMessage {
    fn run(&mut self, client: &mut Game) {
        client.chatbox.println(self.message.as_str());
    }
}

impl<'a> ServerCommand<'a> for EchoMessage {
    fn run(&mut self, (source, server): (&ConnectionID, &mut Server)) {
        server.connection.send(vec![*source], self);
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatMessage {
    message: String
}

impl ChatMessage {
    pub fn new(message: String) -> Self {
        ChatMessage {
            message
        }
    }
}

impl<'a> ClientCommand<'a> for ChatMessage {
    fn run(&mut self, client: &mut Game) {
        client.chatbox.println(self.message.as_str());
    } 
}

impl<'a> ServerCommand<'a> for ChatMessage {
    fn run(&mut self, (id, server): (&ConnectionID, &mut Server)) {
        // reformat this message to include the sender's name
        // for now we just make the name their address
        let name = match server.connection.get_address(id) {
            None => return,
            Some(addr) => addr.to_string()
        };
        let message = format!("<{}> {}", name, self.message);
        server.connection.send(server.connection.all_connection_ids(), &ChatMessage::new(message));
    }
}

#[derive(Clone)]
pub enum State {
    DEFAULT,
    TYPING
}

pub struct Game<'a> {
    pub window_size: Vector2<i32>,
    pub ortho: Orthographic3<f32>,
    pub chatbox: chatbox::Chatbox<'a>,
    pub state: State,
    pub connection: networking::client::Connection,
    pub world: World
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
            connection: networking::client::Connection::new(default_server).unwrap(),
            world: World::new()
        };
        game.window_size(width, height);
        let render = graphics::textured::Renderer::new_square();

        // let mut texture_library = graphics::TextureLibrary::new();
        // let texture = texture_library.make_texture("tree.png");
        game.chatbox.println("Hello");

        let fontinfo = graphics::text::make_font(&font_library, "arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));
        let font_texture = graphics::make_texture(fontinfo.image_size.x as i32, fontinfo.image_size.y as i32, &graphics::text::convert_r_to_rgba(&fontinfo.image_buffer));

        let mut selected_char: Option<CharacterID> = None;

//        let mut temp_id_gen = CharacterIDGenerator::new();
//        GenerateCharacter::generate_character(&mut game.world, &mut temp_id_gen);

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

            // update connection
            game.connection.flush(); // send messages
            let messages = game.connection.poll_raw();
            game.execute(messages);

            // update logic
            let key_dir = {
                let w = match window.get_key(Key::W) { Action::Press => 1.0, _ => 0.0 };
                let s = match window.get_key(Key::S) { Action::Press => 1.0, _ => 0.0 };
                let a = match window.get_key(Key::A) { Action::Press => 1.0, _ => 0.0 };
                let d = match window.get_key(Key::D) { Action::Press => 1.0, _ => 0.0 };
                let mut v = Vector2::<f32>::new(d - a, s - w);
                if v.magnitude_squared() > 0.0 {
                    v.normalize_mut();
                }
                v
            };
            if let Some(char_id) = selected_char {
                if let Some(base) = game.world.base.components.get_mut(&char_id) {
                    base.position += key_dir * delta_time as f32 * 100.0;
                }
            }

            // random text
            let sim = Similarity3::<f32>::new(
                Vector3::new(100.0, 500.0, 0.0),
                Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                1.0
            );
            let matrix = game.ortho.as_matrix() * sim.to_homogeneous();
            let msg = String::from("Hihfas \u{2122} dhofhoas dohfaho hoh7o  H&AH&*( (&*DF(&SD(&*F&*(SD^*(F(&^!)*#$^&$^!_$^)$&*)RUHR\"");
            let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
            text.render(&matrix, msg.as_str(), &color);

            // font spritesheet
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

            // characters
            for cid in &game.world.characters {
                if let Some(base) = game.world.base.components.get(cid) {
                    let sim = Similarity3::<f32>::new(
                        Vector3::new(base.position.x, base.position.y, 0.0),
                        Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                        100.0
                    );
                    let color = match Some(*cid) == selected_char {
                        true => Vector4::new(1.0, 0.0, 0.0, 1.0),
                        false => Vector4::new(1.0, 1.0, 1.0, 1.0)
                    };
                    simple_render.render(
                        &(game.ortho.as_matrix() * sim.to_homogeneous()),
                        &color,
                        graphics::VertexRange::Full
                    );
                }
            }

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
                        // game.chatbox.add_typing('/'); // this gets added automatically lol
                    },
                    (State::DEFAULT, glfw::WindowEvent::Key(Key::G, _, Action::Press, _)) => {
                        game.world.characters.iter().map(
                            |id| game.world.make_cmd_update_character(*id)
                        ).for_each(|cmd| match cmd {
                            Some(cmd) => game.connection.send(&cmd),
                            _ => ()
                        });
                    },
                    (State::DEFAULT, glfw::WindowEvent::Key(Key::Tab, _, Action::Press, _)) => {
                        let ids: Vec<&CharacterID> = game.world.characters.iter().collect();
                        selected_char = match ids.len() {
                            0 => None,
                            _ => {
                                let index = match selected_char {
                                    None => 0,
                                    Some(id) => {
                                        let mut index = 0;
                                        for i in 0..ids.len() {
                                            if *ids[i] == id {
                                                index = (i + 1) % ids.len();
                                                break;
                                            }
                                        }
                                        index
                                    }
                                };
                                Some(*ids[index])
                            }
                        }
                    },
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
                ["send", _, ..] => {
                    self.connection.send(&ChatMessage::new(String::from(&command[("send ".len() + 1)..])));
                },
                ["echo", _, ..] => {
                    self.connection.send(&EchoMessage::new(String::from(&command[("echo ".len() + 1)..])));
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
                },
                ["stopserver"] => {
                    self.connection.send(&StopServer());
                },
                ["genchar"] => {
                    self.connection.send(&GenerateCharacter::new());
                },
                _ => self.chatbox.println("Failed to parse command.")
            }
        }
    }
}
