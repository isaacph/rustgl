extern crate glfw;

pub mod graphics;

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

use std::{net::{TcpListener, SocketAddr}, mem::MaybeUninit, io::{ErrorKind, IoSlice, Read}};
use socket2::{Socket, Domain, Type, Protocol};

use std::io::Result;

use std::env;

pub mod networking {
    pub mod server {
        use socket2::SockAddr;
        use std::collections::HashMap;
        use std::io::Result;
        use std::io::Error;
        use std::{mem::MaybeUninit, io::{ErrorKind, IoSlice, Read}};
        use std::time::{SystemTime, Duration};
        use socket2::{Socket, Domain, Type, Protocol};
        use std::net::SocketAddr;

        // the time after the last message after which to declare the client dead
        const LAST_MESSAGE_DEAD_TIME: Duration = Duration::new(10, 0);

        struct ClientInfo {
            id: ClientID,
            address: SocketAddr,
            send_queue: Vec<Vec<u8>>,
            last_message: SystemTime
        }

        #[derive(PartialEq, Eq, Hash, Clone)]
        pub struct ClientID(i32);

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

        pub enum PollResult {
            Ok(HashMap<ClientID, Vec<Vec<u8>>>),
            Err((HashMap<ClientID, Vec<Vec<u8>>>, Error))
        }

        struct ServerConnection {
            socket: Socket,
            client_data: HashMap<ClientID, ClientInfo>,
            addr_id_map: HashMap<SocketAddr, ClientID>,
            generator: ClientIDGenerator
        }

        impl ServerConnection {
            pub fn new(port: u16) -> Result<ServerConnection> {
                let socket = make_socket(Some(port))?;
                Ok(ServerConnection {
                    socket,
                    client_data: HashMap::new(),
                    addr_id_map: HashMap::new(),
                    generator: ClientIDGenerator::new()
                })
            }

            fn new_client(&mut self, addr: &SocketAddr) -> ClientID {
                let id = self.generator.generate();
                self.addr_id_map.insert(*addr, id.clone());
                self.client_data.insert(id.clone(), ClientInfo {
                    id: id.clone(),
                    address: *addr,
                    send_queue: Vec::new(),
                    last_message: SystemTime::now()
                });
                id
            }

            pub fn poll(&mut self) -> PollResult {
                let mut buffer = [MaybeUninit::<u8>::uninit(); 1024];
                let mut messages: HashMap<ClientID, Vec<Vec<u8>>> = HashMap::new();
                let mut error: Option<Error> = None;
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
                            let result: &[u8] = unsafe {
                                let temp: &[u8; 1024] = std::mem::transmute(&buffer);
                                &temp[0..size]
                            };
                            let id = match self.addr_id_map.get(&addr) {
                                Some(id) => *id,
                                None => self.new_client(&addr)
                            };
                            let msg_list = match messages.get_mut(&id) {
                                Some(list) => list,
                                None => match messages.try_insert(id, Vec::new()) {
                                    Ok(list) => list,
                                    Err(e) => {
                                        println!("Error adding new client message");
                                        continue
                                    }
                                }
                            };
                            println!("Received {} bytes from {:?}", size, addr);
                            println!("Message: {}", &std::str::from_utf8(result).unwrap());
                        }
                        Err(v) => {
                            match v.kind() {
                                ErrorKind::WouldBlock => (),
                                _ => error = Some(v)
                            }
                            break;
                        }
                    };
                }
                match error {
                    None => PollResult::Ok(messages),
                    Some(v) => PollResult::Err((messages, v))
                }
            }
            pub fn flush(&mut self) -> Result<()> {
                Ok(())
            }
            pub fn send(&mut self, client: ClientID, data: Vec<u8>) {
            }
            pub fn connected(&self, client: ClientID) -> bool {
                false
            }
        }

        fn make_socket(port: Option<u16>) -> Result<Socket> {
            let socket: Socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
            //socket.set_nonblocking(true)?;
            socket.set_reuse_address(true)?;
            let address: SocketAddr = format!("127.0.0.1:{}",
                match port {Some(port) => port, _ => 0}
            ).parse().unwrap();
            let address = address.into();
            socket.bind(&address)?;

            Ok(socket)
        }
    }
}

fn main() -> Result<()> {
    // game::Game::run();
    let args: Vec<String> = env::args().collect();
    let port_str = args.get(1).unwrap();

//    let socket = make_socket(None).unwrap();
//    println!("Local address: {:?}", socket.local_addr().unwrap().as_socket_ipv4().unwrap());
//
//    let address: SocketAddr = format!("127.0.0.1:{}", port_str).parse().unwrap();
//    let address = address.into();
//    let written = socket.send_to_vectored(&[IoSlice::new(b"Hello world\0")], &address).unwrap();
//    println!("Bytes written: {}", written);

//    let mut buffer = [MaybeUninit::<u8>::uninit(); 1024];
//    let mut recvd = false;
//    while !recvd {
//        let res = socket.recv_from(buffer.as_mut_slice());
//        match res {
//            Ok((size, addr)) => {
//                recvd = true;
//                let result: &[u8] = unsafe {
//                    let temp: &[u8; 1024] = std::mem::transmute(&buffer);
//                    &temp[0..size]
//                };
//                println!("Received {} bytes from {:?}", size, addr);
//                println!("Message: {}", &std::str::from_utf8(result).unwrap());
//            }
//            Err(v) => {
//                match v.kind() {
//                    ErrorKind::WouldBlock => (),
//                    _ => println!("Error: {}", v),
//                }
//            }
//        }
//    }
    Ok(())
}

