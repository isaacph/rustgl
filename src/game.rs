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
        StopServer, EmptyCommand},
    networking_wrapping::{
        ClientCommand,
        ServerCommand, SerializedClientCommand, SerializedServerCommand,
	},
    world::{
        World,
        GenerateCharacter,
        character::CharacterID, player::{PlayerLogIn, PlayerLogOut}
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
    fn run(self, client: &mut Game) {
        client.chatbox.println(self.message.as_str());
    }
}

impl<'a> ServerCommand<'a> for EchoMessage {
    fn run(self, (source, server): (&ConnectionID, &mut Server)) {
        let ser = SerializedClientCommand::from(&self);
        server.connection.send_raw(vec![*source], ser.data);
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
    fn run(self, client: &mut Game) {
        client.chatbox.println(self.message.as_str());
    } 
}

impl<'a> ServerCommand<'a> for ChatMessage {
    fn run(mut self, (id, server): (&ConnectionID, &mut Server)) {
        // reformat this message to include the sender's name or IP if they aren't signed in
        let name = match server.player_manager.get_connected_player(id) {
            None => match server.connection.get_address(id) {
                None => return,
                Some(addr) => format!("From {}", addr.to_string())
            },
            Some(player) => player.name.clone()
        };
        self.message = format!("<{}> {}", name, self.message);
        let ser = SerializedClientCommand::from(&self);
        server.connection.send_raw(server.connection.all_connection_ids(), ser.data);
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

        game.connection.send_raw(SerializedServerCommand::from(&EmptyCommand).data);
        let mut last_heartbeat = glfw.get_time();

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
            // heartbeat to avoid disconnection
            if current_time - last_heartbeat > 1.0 {
                game.connection.send_raw(SerializedServerCommand::from(&EmptyCommand).data);
                last_heartbeat = current_time;
            }
            game.connection.flush(); // send messages
            let messages = game.connection.poll_raw();
            for data in messages {
                let ser_cmd = SerializedClientCommand::new(data);
                ser_cmd.execute(&mut game);
            }

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
                            match game.process_chat(line.as_str()) {
                                Ok(Some(message)) => game.chatbox.println(message.as_str()),
                                Ok(None) => (),
                                Err(message) => game.chatbox.println(message.as_str())
                            }
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
                            Some(cmd) => game.connection.send_raw(SerializedServerCommand::from(&cmd).data),
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

    pub fn process_chat(&mut self, command: &str) -> Result<Option<String>, String> {
        if !command.starts_with('/') {
            self.process_chat((String::from("/send ") + command).as_str())
        } else {
            let split: Vec<&str> = command[1..].split(' ').collect();
            match &split[..] {
                ["hello", "world"] => Ok(Some(format!("Hello world!"))),
                ["send", _, ..] => {
                    let ser = SerializedServerCommand::from(&ChatMessage::new(String::from(&command[("send ".len() + 1)..])));
                    self.connection.send_raw(ser.data);
                    Ok(None)
                },
                ["echo", _, ..] => {
                    let ser = SerializedServerCommand::from(&EchoMessage::new(String::from(&command[("echo ".len() + 1)..])));
                    self.connection.send_raw(ser.data);
                    Ok(None)
                },
                ["print", _, ..] => Ok(Some(String::from(&command[("/print ".len())..]))),
                ["server", address] => {
                    let address: SocketAddr = match address.parse() {
                        Err(e) => return Err(format!("Error parsing address: {}", e)),
                        Ok(address) => address
                    };
                    self.connection.set_server_address(&address);
                    Ok(Some(format!("Successfully changed server address to {}", address.to_string())))
                },
                ["stopserver"] => {
                    let ser = SerializedServerCommand::from(&StopServer());
                    self.connection.send_raw(ser.data);
                    Ok(Some(format!("Stop command sent")))
                },
                ["genchar"] => {
                    let ser = SerializedServerCommand::from(&GenerateCharacter::new());
                    self.connection.send_raw(ser.data);
                    Ok(Some(format!("Character gen command sent")))
                },
                ["login", typ, ..] => {
                    self.connection.send_raw(
                        SerializedServerCommand::from(&PlayerLogIn {
                            existing: match *typ {
                                "new" => false,
                                "old" => true,
                                _ => return Err(format!(
                                    "Unknown login type: {}. Options are: new old",
                                    typ
                                ))
                            },
                            name: match split.len() {
                                2 => None,
                                n => Some(String::from(split[2..n].join(" "))),
                            }
                        }).data
                    );
                    Ok(None)
                },
                ["logout"] => {
                    self.connection.send_raw(
                        SerializedServerCommand::from(&PlayerLogOut).data
                    );
                    Ok(None)
                }
                _ => Err(format!("Failed to parse command."))
            }
        }
    }
}
