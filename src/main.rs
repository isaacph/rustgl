extern crate glfw;

pub mod graphics;

pub mod chatbox {
    use std::iter::Rev;

    use nalgebra::{Matrix4, Vector3, Vector4};

    use crate::graphics::text::*;
    pub struct Chatbox<'a> {
        font: &'a Font,
        visible_lines: i32,
        history_length: i32,
        typing: String,
        history: Vec<String>,
        width: f32,
        timer: f32
    }

    pub const BAR_FLICKER_TIME: f32 = 1.0;

    impl Chatbox<'_> {
        pub fn new<'a>(font: &'a Font, visible_lines: i32, history_length: i32, width: f32) -> Chatbox<'a> {
            assert!(visible_lines >= 0 && history_length >= 0 && width >= 0.0);
            Chatbox::<'a> {
                font,
                visible_lines,
                history_length,
                typing: String::new(),
                history: Vec::new(),
                width,
                timer: 0.0
            }
        }

        pub fn println(&mut self, line: &str) {
            let mut lines: Vec<String> = self.font.split_lines(line, Some(self.width));
            let add_len = std::cmp::min(self.history_length as usize, lines.len());
            lines.drain(0..(lines.len() - add_len));
            let history_remove = self.history.len() - add_len;
            self.history.drain(0..history_remove);
            self.history.append(&mut lines);
        }

        pub fn get_visible_history(&self) -> Rev<std::slice::Iter<'_, String>> {
            self.history.as_slice()[(self.history.len() - self.visible_lines as usize)..self.history.len()].iter().rev()
        }

        pub fn get_typing(&self) -> &String {
            &self.typing
        }

        pub fn render(&mut self, proj: Matrix4<f32>, delta_time: f32) {
            let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
            let matrix = self.get_visible_history().fold(proj, |matrix, line| {
                self.font.render(&matrix, line.as_str(), &color);
                matrix.append_translation(&Vector3::new(0.0, self.font.line_height(), 0.0))
            });

            self.timer += delta_time;
            while self.timer > BAR_FLICKER_TIME {
                self.timer -= BAR_FLICKER_TIME;
            }
            let typing_line = if self.timer > BAR_FLICKER_TIME / 2.0 {
                self.typing.to_owned() + "|"
            } else {
                self.typing.to_owned()
            };
            self.font.render(&matrix, typing_line.as_str(), &color);
        }
    }
}

mod game {
    use nalgebra::{Vector2, Orthographic3};
    use ogl33::glViewport;

    use std::ffi::CStr;
    use glfw::{Action, Context, Key};
    use nalgebra::{Vector4, Vector3, Similarity3};
    use ogl33::*;

    use crate::graphics;

    pub struct Game {
        pub window_size: Vector2<i32>,
        pub ortho: Orthographic3<f32>
    }

    impl Game {
        pub fn run() {
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
            
            let mut game = Game::new(width, height);
            let render = graphics::textured::Renderer::new_square();

            // let mut texture_library = graphics::TextureLibrary::new();
            // let texture = texture_library.make_texture("tree.png");
            let mut font_library = graphics::text::FontLibrary::new();
            let text = font_library.make_font("arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));

            let fontinfo = graphics::text::make_font(&font_library, "arial.ttf", 32, graphics::text::default_characters().iter(), Some('\0'));
            let font_texture = graphics::make_texture(fontinfo.image_size.x as i32, fontinfo.image_size.y as i32, &graphics::text::convert_r_to_rgba(&fontinfo.image_buffer));

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
        
        pub fn new(width: i32, height: i32) -> Game {
            let mut game = Game {
                window_size: Vector2::<i32>::new(width, height),
                ortho: Orthographic3::<f32>::new(0.0, width as f32, height as f32, 0.0, 0.0, 1.0)
            };
            game.window_size(width, height);
            game
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

pub mod networking {
    use std::net::SocketAddr;

    use socket2::{Socket, Domain, Type, Protocol};
    use std::io::Result;

    fn make_socket(port: Option<u16>) -> Result<Socket> {
        let socket: Socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
        socket.set_nonblocking(true)?;
        socket.set_reuse_address(true)?;
        let address: SocketAddr = format!("127.0.0.1:{}",
            match port {Some(port) => port, _ => 0}
        ).parse().unwrap();
        let address = address.into();
        socket.bind(&address)?;

        Ok(socket)
    }
    
    pub mod server {
        use std::collections::HashMap;
        use std::fmt::Display;
        use std::io::Result;
        use std::mem::MaybeUninit;
        use std::time::SystemTime;
        use socket2::Socket;
        use std::net::SocketAddr;

        use super::make_socket;

        // the time after the last message after which to declare the client dead
        // const LAST_MESSAGE_DEAD_TIME: Duration = Duration::new(10, 0);

        struct ClientInfo {
            address: SocketAddr,
            last_message: SystemTime
        }

        #[derive(PartialEq, Eq, Hash, Clone, Copy)]
        pub struct ClientID(i32);

        impl Display for ClientID {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        struct ClientIDGenerator {
            counter: i32
        }

        impl ClientIDGenerator {
            fn new() -> Self {
                Self {
                    counter: 0
                }
            }
            fn generate(&mut self) -> ClientID {
                self.counter += 1;
                ClientID(self.counter - 1)
            }
        }

        pub struct ServerConnection {
            socket: Socket,
            client_data: HashMap<ClientID, ClientInfo>,
            addr_id_map: HashMap<SocketAddr, ClientID>,
            generator: ClientIDGenerator,
            message_queue: Vec<(ClientID, Vec<u8>)>,
        }

        impl ServerConnection {
            pub fn new(port: u16) -> Result<ServerConnection> {
                let socket = make_socket(Some(port))?;
                Ok(ServerConnection {
                    socket,
                    client_data: HashMap::new(),
                    addr_id_map: HashMap::new(),
                    generator: ClientIDGenerator::new(),
                    message_queue: Vec::new()
                })
            }

            fn new_client(&mut self, addr: &SocketAddr) -> ClientID {
                let id = self.generator.generate();
                self.addr_id_map.insert(*addr, id.clone());
                self.client_data.insert(id.clone(), ClientInfo {
                    address: *addr,
                    last_message: SystemTime::now()
                });
                id
            }

            pub fn get_address(&self, id: &ClientID) -> Option<SocketAddr> {
                match self.client_data.get(id) {
                    Some(data) => Some(data.address),
                    None => None
                }
            }

            pub fn poll(&mut self) -> HashMap<ClientID, Vec<Vec<u8>>> {
                let mut buffer = [MaybeUninit::<u8>::uninit(); 16384];
                let mut messages: HashMap<ClientID, Vec<Vec<u8>>> = HashMap::new();
                loop {
                    match self.socket.recv_from(buffer.as_mut_slice()) {
                        Ok((size, addr)) => {
                            let addr: SocketAddr = match addr.as_socket() {
                                Some(addr) => addr,
                                None => {
                                    println!("Error understanding connection address");
                                    continue
                                }
                            };
                            let result: Vec<u8> = unsafe {
                                let temp: &[u8; 1024] = std::mem::transmute(&buffer);
                                &temp[0..size]
                            }.to_vec();
                            println!("Received {} bytes from {:?}: {}", size, addr, std::str::from_utf8(&result).unwrap());
                            let id = match self.addr_id_map.get(&addr) {
                                Some(id) => *id,
                                None => self.new_client(&addr)
                            };
                            match self.client_data.get_mut(&id) {
                                Some(data) => data.last_message = SystemTime::now(),
                                None => panic!("Error: client found in address map but not in data map: {}", id)
                            }
                            match messages.get_mut(&id) {
                                Some(list) => list.push(result),
                                None => {
                                    messages.insert(id, vec![result]);
                                }
                            };
                        }
                        Err(v) => {
                            match v.kind() {
                                std::io::ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error reading message: {}", v);
                                }
                            }
                            break;
                        }
                    };
                }
                messages
            }
            pub fn flush(&mut self) {
                let mut failed: Vec<(ClientID, Vec<u8>)> = Vec::new();
                for (id, data) in self.message_queue.drain(0..self.message_queue.len()) {
                    let client_data = match self.client_data.get(&id) {
                        Some(data) => data,
                        None => {
                            println!("Error sending data to unknown client (id={})", id);
                            continue;
                        }
                    };
                    let addr = client_data.address.into();
                    let written = self.socket.send_to(data.as_slice(), &addr);
                    match written {
                        Ok(written) => {
                            if written != data.len() {
                                println!("Error writing {} bytes to {}: {} < {}", data.len(), client_data.address, written, data.len());
                            }
                        },
                        Err(e) => {
                            match e.kind() {
                                std::io::ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error writing {} bytes to {}: {}", data.len(), client_data.address, e);
                                    failed.push((id, data));
                                    continue;
                                }
                            }
                        }
                    }
                }
                self.message_queue.append(&mut failed);
            }
            pub fn send(&mut self, client: &ClientID, data: Vec<u8>) {
                self.message_queue.push((*client, data));
            }
            pub fn last_message_time(&self, client: &ClientID) -> Option<SystemTime> {
                match self.client_data.get(&client) {
                    Some(data) => Some(data.last_message),
                    None => None
                }
            }
        }
    }

    pub mod client {
        use std::io::Result;
        use std::mem::MaybeUninit;
        use socket2::{Socket, SockAddr};
        use std::net::SocketAddr;
        use std::io::ErrorKind;

        use super::make_socket;

        pub struct Connection {
            socket: Socket,
            server_address: SockAddr,
            server_address_socket: SocketAddr,
            message_queue: Vec<Vec<u8>>
        }

        impl Connection {
            pub fn new(server_address: &SocketAddr) -> Result<Connection> {
                let socket = make_socket(None)?;
                Ok(Connection {
                    socket,
                    server_address: (*server_address).into(),
                    server_address_socket: *server_address,
                    message_queue: Vec::new()
                })
            }

            pub fn get_server_address(&self) -> SocketAddr {
                self.server_address_socket
            }

            pub fn poll(&mut self) -> Vec<Vec<u8>> {
                let mut buffer = [MaybeUninit::<u8>::uninit(); 16384];
                let mut messages: Vec<Vec<u8>> = Vec::new();
                loop {
                    match self.socket.recv_from(buffer.as_mut_slice()) {
                        Ok((size, addr)) => {
                            let addr: SocketAddr = match addr.as_socket() {
                                Some(addr) => addr,
                                None => {
                                    println!("Error understanding connection address");
                                    continue
                                }
                            };
                            let result: Vec<u8> = unsafe {
                                let temp: &[u8; 1024] = std::mem::transmute(&buffer);
                                &temp[0..size]
                            }.to_vec();
                            println!("Received {} bytes from {:?}: {}", size, addr, std::str::from_utf8(&result).unwrap());
                            messages.push(result);
                        }
                        Err(v) => {
                            match v.kind() {
                                ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error reading message: {}", v);
                                }
                            }
                            break;
                        }
                    };
                }
                messages
            }
            pub fn flush(&mut self) {
                let mut failed: Vec<Vec<u8>> = Vec::new();
                for data in self.message_queue.drain(0..self.message_queue.len()) {
                    let addr = &self.server_address;
                    let written = self.socket.send_to(data.as_slice(), addr);
                    match written {
                        Ok(written) => {
                            if written != data.len() {
                                println!("Error writing {} bytes to server: {} < {}", data.len(), written, data.len());
                            }
                        },
                        Err(e) => {
                            match e.kind() {
                                ErrorKind::WouldBlock => (),
                                _ => {
                                    println!("Error writing {} bytes to server: {}", data.len(), e);
                                    failed.push(data);
                                    continue;
                                }
                            }
                        }
                    }
                }
                self.message_queue.append(&mut failed);
                // match error {
                //     None => {
                //         self.message_queue.clear();
                //         Ok(())
                //     },
                //     Some(err) => {
                //         self.message_queue = self.message_queue.split_off(stopped);
                //         Err(err)
                //     }
                // }
            }
            pub fn send(&mut self, data: Vec<u8>) {
                self.message_queue.push(data);
            }
        }
    }
}

use std::{io::Result, net::SocketAddr, time::Duration};
use std::env;
use std::thread;
use std::sync::mpsc;
use std::sync::mpsc::TryRecvError;
use std::io;

fn echo_server(port: u16) -> Result<()> {
    let mut server = networking::server::ServerConnection::new(port)?;
    let mut stop = false;
    while !stop {
        for (id, data) in server.poll() {
            for packet in data {
                if std::str::from_utf8(packet.as_slice()).unwrap().eq("stop") {
                    stop = true;
                }
                server.send(&id, packet);
            }
        }
        server.flush();
        std::thread::sleep(Duration::new(0, 1000000 * 500)); // wait 500 ms
    }
    Ok(())
}

fn console_client(address: SocketAddr) -> Result<()> {
    let mut client = networking::client::Connection::new(&address)?;
    let stdin_channel = {
        let (tx, rx) = mpsc::channel::<String>();
        thread::spawn(move || loop {
            let mut buffer = String::new();
            io::stdin().read_line(&mut buffer).unwrap();
            tx.send(buffer).unwrap();
        });
        rx
    };
    loop {
        for packet in client.poll() {
            println!("Received from server: {}", std::str::from_utf8(packet.as_slice()).unwrap());
        }
        client.flush();
        let message = match stdin_channel.try_recv() {
            Ok(v) => v,
            Err(TryRecvError::Empty) => continue,
            Err(TryRecvError::Disconnected) => break,
        };
        client.send(Vec::from(message.as_bytes()));
        std::thread::sleep(Duration::new(0, 1000000 * 100)); // wait 100 ms
    }
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    match args[1].as_str() {
        "server" => {
            echo_server(1234)
        },
        "gclient" => {
            game::Game::run();
            Ok(())
        }
        _ => { // client
            let server_address: SocketAddr = "127.0.0.1:1234".parse().unwrap();
            console_client(server_address)
        }
    }
}

