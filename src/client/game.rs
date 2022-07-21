use nalgebra::Vector2;
use ogl33::glViewport;
use std::{ffi::CStr, str::FromStr, net::SocketAddr, collections::{HashMap, BTreeMap}};
use std::ops::Bound::{Included, Unbounded};
use glfw::{Action, Context, Key};
use nalgebra::{Vector4, Vector3, Similarity3};
use ogl33::*;
use crate::{
    graphics,
    client::{chatbox, commands::execute_client_command, camera::{CameraContext, CameraMatrix}},
    model::{world::{
        World,
        character::{CharacterID, CharacterType}, commands::{GenerateCharacter, ListChar, EnsureCharacter}, system::{movement::MoveCharacterRequest, auto_attack::AutoAttackRequest}, component::CharacterFlip,
    }, commands::core::GetAddress, Subscription, PrintError, player::{commands::{PlayerSubs, PlayerSubCommand, PlayerLogIn, PlayerLogOut, ChatMessage, GetPlayerData}, model::{PlayerID, PlayerDataView}}}, networking::{client::ClientUpdate, Protocol},
};

use crate::networking::client::Client as Connection;

use super::commands::SendCommands;

#[derive(Clone)]
pub enum State {
    DEFAULT,
    TYPING
}

pub struct Game<'a> {
    pub window_size: Vector2<i32>,
    pub chatbox: chatbox::Chatbox<'a>,
    pub state: State,
    pub world: World,
    pub connection: Connection,
    pub finding_addr: bool,
    pub finding_addr_timer: f32,
    pub mouse_pos: Vector2<f32>,
    pub mouse_pos_world: Vector2<f32>,
    pub move_timer: f32,
    pub selected_player: Option<PlayerID>,
    pub character_name: HashMap<CharacterID, String>,
    pub camera: CameraContext,
    pub ui_scale: f32
}

impl Game<'_> {
    pub fn run(addr: Option<(SocketAddr, SocketAddr)>) {
        let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
        let (start_width, start_height) = (800, 600);

        glfw.window_hint(glfw::WindowHint::ContextVersionMajor(3));
        glfw.window_hint(glfw::WindowHint::ContextVersionMinor(3));
        glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));

        let (mut window, events) = 
            glfw.create_window(start_width as u32, start_height as u32, "Hello Window",
                glfw::WindowMode::Windowed)
                .expect("Failed to create GLFW window.");

        window.set_key_polling(true);
        window.set_char_polling(true);
        window.set_size_polling(true);
        window.set_mouse_button_polling(true);
        window.make_current();

        unsafe {
            load_gl_with(|f_name| {
                let cstr = CStr::from_ptr(f_name);
                let str = cstr.to_str().expect("Failed to convert OGL function name");
                window.get_proc_address(str)
            });
        }

        let mut font_library = graphics::text::FontLibrary::new();
        let mut texture_library = graphics::TextureLibrary::new();
        let text: BTreeMap<i32, graphics::text::Font> = (8..=48).step_by(4).map(
            |i| (i, font_library.make_font(
                "arial.ttf",
                i,
                graphics::text::default_characters().iter(),
                Some('\0'))))
            .collect();
        let simple_render = graphics::simple::Renderer::new_square();
        let texture_render = graphics::textured::Renderer::new_square();
        let mut game = {
            let ui_scale = 32.0;
            Game {
                window_size: Vector2::<i32>::new(start_width, start_height),
                chatbox: chatbox::Chatbox::new({
                    let approx_font_size = ui_scale;
                    match text.range((Included(approx_font_size as i32), Unbounded)).next() {
                        Some((_, font)) => font,
                        None => text.iter().next().expect("No fonts loaded").1
                    }
                }, &simple_render, 7, 40, 800.0),
                state: State::DEFAULT,
                world: World::new(),
                connection: Connection::init_disconnected(),
                finding_addr: true,
                finding_addr_timer: 0.0,
                mouse_pos: Vector2::<f32>::new(0.0, 0.0),
                mouse_pos_world: Vector2::<f32>::new(0.0, 0.0),
                move_timer: 0.0,
                selected_player: None,
                character_name: HashMap::new(),
                camera: CameraContext {
                    width: start_width,
                    height: start_height,
                    position: Vector2::new(0.0, 0.0),
                    zoom: 4.0
                },
                ui_scale
            }
        };

        if let Some((udp_addr, tcp_addr)) = addr {
            game.chatbox.println("Autoconnecting to server...");
            game.connection.connect(udp_addr, tcp_addr);
        }

        game.window_size(start_width, start_height);
        // let render = graphics::textured::Renderer::new_square();

        // let mut texture_library = graphics::TextureLibrary::new();
        // let texture = texture_library.make_texture("tree.png");
        game.chatbox.println("Hello");

        // let fontinfo = graphics::text::make_font(&font_library, "arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));
        // let font_texture = graphics::make_texture(fontinfo.image_size.x as i32, fontinfo.image_size.y as i32, &graphics::text::convert_r_to_rgba(&fontinfo.image_buffer));

        let character_walk_textures: Vec<graphics::Texture> = (1..=12).map(
            |i| texture_library.make_texture(format!("walk_256/Layer {}.png", i).as_str())
        ).collect();

        enum Direction {
            Left, Right
        }
        struct Animation {
            timer: f32,
            direction: Direction
        }
        let mut animation_data: HashMap<CharacterID, Animation> = HashMap::new();
        let animation_fps = 12.0;

        unsafe {
            glClearColor(0.1, 0.2, 0.1, 1.0);
            glEnable(GL_BLEND);
            glBlendFunc(GL_ONE, GL_ONE_MINUS_SRC_ALPHA);
        }

        let mut fpsc: i32 = 0;
        let mut fps: i32 = 0;
        let mut last_fps_time = glfw.get_time();
        let mut last_time = glfw.get_time();
        while !window.should_close() {
            let current_time = glfw.get_time();
            let delta_time = (current_time - last_time) as f32;
            last_time = current_time;
            fpsc += 1;
            if current_time - last_fps_time >= 1.0 {
                fps = fpsc;
                fpsc = 0;
                last_fps_time = current_time;
            }

            unsafe {
                glClear(GL_COLOR_BUFFER_BIT);
            }

            game.mouse_pos = {
                let (x, y) = window.get_cursor_pos();
                Vector2::new(x as f32, y as f32)
            };
            game.mouse_pos_world = game.camera.view_to_world_pos(game.mouse_pos);

            // UDP address pings: send GetAddress -> receive SetAddress ->
            //   falsify game.finding_addr and send SetAddress
            if game.finding_addr && game.connection.is_connected() {
                game.finding_addr_timer -= delta_time;
                if game.finding_addr_timer <= 0.0 {
                    game.connection.send(Protocol::UDP, &GetAddress).print();
                    game.finding_addr_timer = 0.5;
                }
            }
            for update in game.connection.update() {
                match update {
                    ClientUpdate::Error(err) => game.chatbox.println(format!("Connection error: {}", err).as_str()),
                    ClientUpdate::Log(log) => println!("{}", log),
                    ClientUpdate::LogExtra(_) => (), // if you print this, you will get windows
                                                     // alarm spam
                    ClientUpdate::Message(protocol, message) => {
                        match execute_client_command(&message, (protocol, &mut game)) {
                            Ok(()) => (),
                            Err(err) => game.chatbox.println(format!("Error executing message: {}, err: {}", String::from_utf8_lossy(&message), err).as_str())
                        }
                    },
                    _ => game.chatbox.println(format!("{}", update).as_str())
                }
            }
            game.world.update(delta_time);
            for error in game.world.errors.drain(0..game.world.errors.len()) {
                // client side errors usually will be a result of lag
                println!("Client world error: {:?}", error);
            }

            // draw
            let approx_font_size = game.ui_scale;
            let x = (Included(approx_font_size as i32), Unbounded);
            let game_font = match text.range(x).next() {
                Some((_, font)) => font,
                None => {
                    text.iter().next_back().expect("No fonts loaded").1
                }
            };

            let CameraMatrix {
                proj, view
            } = game.camera.matrix();
            let proj_view = proj * view;

            let selected_char = {
                let mut c = None;
                if let Some(pid) = game.selected_player {
                    if let Some(player) = game.world.players.get_player(&pid) {
                        if let Some(cid) = player.selected_char {
                            c = Some(cid)
                        }
                    }
                }
                c
            };

            game.move_timer += delta_time;
            if game.move_timer >= 0.2 && window.get_mouse_button(glfw::MouseButtonRight) == glfw::Action::Press {
                if let Some(pid) = game.selected_player {
                    if let Some(player) = game.world.players.get_player(&pid) {
                        if let Some(cid) = player.selected_char {
                            game.move_timer = 0.0;
                            game.connection.send(Protocol::UDP, &MoveCharacterRequest {
                                id: cid,
                                dest: game.mouse_pos_world,
                            }).ok();
                        }
                    }
                }
            }

            // random text
            // let sim = Similarity3::<f32>::new(
            //     Vector3::new(100.0, 500.0, 0.0),
            //     Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
            //     1.0
            // );
            // let matrix = proj_view * sim.to_homogeneous();
            // let msg = String::from("Hihfas \u{2122} dhofhoas dohfaho hoh7o  H&AH&*( (&*DF(&SD(&*F&*(SD^*(F(&^!)*#$^&$^!_$^)$&*)RUHR\"");
            // let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
            // text.render(&matrix, msg.as_str(), &color);

            // font spritesheet
            // let sim = Similarity3::<f32>::new(
            //     Vector3::new(400.0, 400.0, 0.0),
            //     Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
            //     800.0
            // );
            // render.render(
            //     &(proj * sim.to_homogeneous()),
            //     &Vector4::new(1.0, 1.0, 1.0, 1.0),
            //     &font_texture,
            //     graphics::VertexRange::Full
            // );

            // characters
            for cid in &game.world.characters {
                if let Some(base) = game.world.base.components.get(cid) {
                    match base.ctype {
                        CharacterType::IceWiz => {
                            let Animation { timer: animation_time, direction } = match animation_data.get_mut(cid) {
                                None => {
                                    animation_data.insert(*cid, Animation {
                                        timer: 0.0,
                                        direction: Direction::Right,
                                    });
                                    animation_data.get_mut(cid).unwrap()
                                },
                                Some(time) => time,
                            };
                            let mut frame = 0usize;
                            if let Some(movement) = game.world.movement.components.get(cid) {
                                if let Some(dest) = movement.destination {
                                    *animation_time += delta_time;
                                    *animation_time -= f32::floor(*animation_time * animation_fps / (character_walk_textures.len() as f32))
                                        * character_walk_textures.len() as f32 / animation_fps;
                                    *direction = if dest.x - base.position.x >= 0.0 {
                                        Direction::Right
                                    } else {
                                        Direction::Left
                                    };
                                    frame = (*animation_time * character_walk_textures.len() as f32) as usize;
                                }
                            }
                            let flip_dir: f32 = match base.flip {
                                CharacterFlip::Left => -1.0,
                                CharacterFlip::Right => 1.0
                            };
                            let player_scale = 1.0;
                            let offset = Vector2::new(0.0, -100.0 / 256.0 * player_scale);
                            let matrix = graphics::make_matrix(
                                base.position + offset,
                                Vector2::new(flip_dir * player_scale, player_scale),
                                0.0
                            );
                            let color = match Some(*cid) == selected_char {
                                true => Vector4::new(1.0, 1.0, 1.0, 1.0),
                                false => Vector4::new(1.0, 0.9, 0.9, 1.0)
                            };
                            texture_render.render(
                                &(proj_view * matrix),
                                &color,
                                &character_walk_textures[frame],
                                graphics::VertexRange::Full
                            );

                            // render player name below player
                            if let Some(name) = game.character_name.get(cid) {
                                let text_width = game_font.text_width(name.as_str());
                                let player_view_pos = game.camera.world_to_view_pos(base.position);
                                let offset = Vector2::new(-text_width / 2.0, game_font.line_height());
                                let sim = Similarity3::<f32>::new(
                                    Vector3::new(player_view_pos.x + offset.x, player_view_pos.y + offset.y, 0.0),
                                    Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                                    1.0
                                );
                                game_font.render(&(proj * sim.to_homogeneous()), name.as_str(), &Vector4::new(1.0, 1.0, 1.0, 1.0));
                            }
                        },
                        CharacterType::Projectile => {
                            let scale = 0.2;
                            let matrix = graphics::make_matrix(
                                base.position,
                                Vector2::new(base.flip.dir() * scale, scale),
                                0.0
                            );
                            let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
                            simple_render.render(&(proj_view * matrix), &color, graphics::VertexRange::Full);
                        },
                    };
                }
            }

            game.chatbox.render(&proj, delta_time);

            // show fps
            let msg = format!("FPS: {}  ", fps);
            let fps_width = game_font.text_width(msg.as_str());
            let sim = Similarity3::<f32>::new(
                Vector3::new(game.window_size.x as f32 - fps_width, game_font.line_height(), 0.0),
                Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                1.0
            );
            game_font.render(&(proj * sim.to_homogeneous()), msg.as_str(), &Vector4::new(1.0, 1.0, 1.0, 1.0));

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
                        if !line.is_empty() {
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
                    //(State::DEFAULT, glfw::WindowEvent::Key(Key::Tab, _, Action::Press, _)) => {
                    //    let ids: Vec<&CharacterID> = game.world.characters.iter().collect();
                    //    game.selected_char = match ids.len() {
                    //        0 => None,
                    //        _ => {
                    //            let index = match game.selected_char {
                    //                None => 0,
                    //                Some(id) => {
                    //                    let mut index = 0;
                    //                    for i in 0..ids.len() {
                    //                        if *ids[i] == id {
                    //                            index = (i + 1) % ids.len();
                    //                            break;
                    //                        }
                    //                    }
                    //                    index
                    //                }
                    //            };
                    //            Some(*ids[index])
                    //        }
                    //    }
                    //},
                    (State::DEFAULT, glfw::WindowEvent::MouseButton(glfw::MouseButtonRight, Action::Press, _)) => {
                        if game.connection.is_connected() {
                            if let Some(pid) = game.selected_player {
                                if let Some(player) = game.world.players.get_player(&pid) {
                                    if let Some(cid) = player.selected_char {
                                        game.move_timer = 0.0;
                                        game.connection.send(Protocol::UDP, &MoveCharacterRequest {
                                            id: cid,
                                            dest: game.mouse_pos_world,
                                        }).ok();
                                    }
                                }
                            }
                        }
                    }
                    (State::DEFAULT, glfw::WindowEvent::MouseButton(glfw::MouseButtonLeft, Action::Press, _)) => {
                        if game.connection.is_connected() {
                            if let Some(pid) = game.selected_player {
                                if let Some(player) = game.world.players.get_player(&pid) {
                                    if let Some(cid) = player.selected_char {
                                        let mouse_pos_world = game.mouse_pos_world;
                                        if let (Some(target), _) = game.world.base.components.iter().fold((None, f32::MAX), |(mut cur_cid, mut dist), (ncid, base)| {
                                            let mag = (mouse_pos_world - base.position).magnitude();
                                            if mag < dist && cid != *ncid {
                                                cur_cid = Some(*ncid);
                                                dist = mag;
                                            }
                                            (cur_cid, dist)
                                        }) {
                                            game.connection.send(Protocol::UDP, &AutoAttackRequest {
                                                attacker: cid,
                                                target
                                            }).ok();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    pub fn window_size(&mut self, width: i32, height: i32) {
        self.window_size.x = width;
        self.window_size.y = height;
        self.camera.width = width;
        self.camera.height = height;
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
                ["hello", "world"] => Ok(Some("Hello world!".to_string())),
                ["connect", addr_udp, addr_tcp] => match (addr_udp.parse(), addr_tcp.parse()) {
                    (Ok(addr_udp), Ok(addr_tcp)) => {
                        let addr_udp: SocketAddr = addr_udp;
                        let addr_tcp: SocketAddr = addr_tcp;
                        self.connection.connect(addr_udp, addr_tcp);
                        self.finding_addr = true;
                        self.finding_addr_timer = 0.0;
                        Ok(Some(format!("Starting connection with {}, {}", addr_udp, addr_tcp)))
                    },
                    (Err(err), _) | (_, Err(err)) => Err(format!("{}", err))
                },
                ["purelogin", ..] => {
                    let existing = if split.len() >= 2 {
                        match split[1] {
                            "new" => false,
                            "old" => true,
                            err => return Err(format!("Unknown login type: {}", err))
                        }
                    } else { false };
                    let name = {
                        if split.len() >= 3 {
                            let x = split[2..].join(" ");
                            if !x.is_empty() {
                                Some(x)
                            } else { None }
                        } else {
                            None
                        }
                    };
                    self.connection.send(Protocol::TCP, &PlayerLogIn {existing, name})?;
                    Ok(None)
                },
                ["login", ..] => {
                    let existing = if split.len() >= 2 {
                        match split[1] {
                            "new" => false,
                            "old" => true,
                            err => return Err(format!("Unknown login type: {}", err))
                        }
                    } else { false };
                    let name = {
                        if split.len() >= 3 {
                            let x = split[2..].join(" ");
                            if !x.is_empty() {
                                Some(x)
                            } else { None }
                        } else {
                            None
                        }
                    };
                    self.connection.send(Protocol::TCP, &PlayerLogIn {existing, name})?;
                    self.connection.send(Protocol::TCP, &PlayerSubs(PlayerSubCommand::SetSubs(vec![Subscription::Chat, Subscription::World])))?;
                    self.connection.send(Protocol::TCP, &EnsureCharacter)?;
                    Ok(None)
                },
                ["logout", ..] => {
                    self.connection.send(Protocol::TCP, &PlayerLogOut)?;
                    Ok(None)
                },
                ["send", _, ..] => {
                    self.connection.send(Protocol::TCP, &ChatMessage(command["send ".len()..].into()))?;
                    Ok(None)
                },
                ["get", "players"] => {
                    self.connection.send(Protocol::TCP, &GetPlayerData)?;
                    Ok(None)
                },
                ["gen", "char"] => {
                    self.connection.send(Protocol::TCP, &GenerateCharacter(CharacterType::IceWiz))?;
                    Ok(None)
                }
                ["sub"] | ["sub", "list", ..] => {
                    self.connection.send(Protocol::TCP, &PlayerSubs(PlayerSubCommand::ListSubs))?;
                    Ok(None)
                },
                ["sub", op, ..] => {
                    let list: Result<Vec<Subscription>, String> = match split.len() {
                        0 | 1 | 2 => Ok(vec![]),
                        _ => split[2..].iter().fold(Ok(vec![]), |acc, n| match acc {
                            Ok(mut acc) => match Subscription::from_str(*n) {
                                Ok(s) => {
                                    acc.push(s);
                                    Ok(acc)
                                },
                                Err(e) => Err(e.to_string())
                            },
                            x => x
                        }),
                    };
                    match (*op, list) {
                        ("add", Ok(list)) => {
                            self.connection.send(Protocol::TCP, &PlayerSubs(PlayerSubCommand::AddSubs(list)))?;
                            Ok(None)
                        },
                        ("del", Ok(list)) => {
                            self.connection.send(Protocol::TCP, &PlayerSubs(PlayerSubCommand::DelSubs(list)))?;
                            Ok(None)
                        },
                        ("set", Ok(list)) => {
                            self.connection.send(Protocol::TCP, &PlayerSubs(PlayerSubCommand::SetSubs(list)))?;
                            Ok(None)
                        },
                        (_, Err(e)) => Err(format!("Error parsing list: {}", e)),
                        _ => Err(format!("Invalid option: {}", op))
                    }
                },
                ["listchar"] => {
                    self.connection.send(Protocol::TCP, &ListChar)?;
                    Ok(Some(format!("Selected: {}\nLocal:\n{}", {
                        if let Some(player) = self.selected_player {
                            match self.world.players.get_player(&player) {
                                None => format!("Selected player not found: {:?}", player),
                                Some(player) => format!("{:?}", player.selected_char)
                            }
                        } else {
                            "No player selected".to_string()
                        }
                    }, {
                        let x: Vec<String> = self.world.characters.iter().map(
                            |cid| format!(
                                "{:?}: components: {:?}",
                                // base: {:?} health: {:?} move: {:?} icewiz: {:?}",
                                cid,
                                self.world.get_components(cid),
                                // self.world.base.components.get(cid),
                                // self.world.health.components.get(cid),
                                // self.world.movement.components.get(cid),
                                // self.world.icewiz.components.get(cid))
                            )
                        ).collect();
                        x.join(", ")
                    })))
                },
                _ => Err("Unknown command or incorrect parameters.".to_string())
            }
        }
    }
}
