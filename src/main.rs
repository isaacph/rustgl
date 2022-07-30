// #![windows_subsystem = "windows"]

#[cfg(feature = "client")]
extern crate glfw;

#[cfg(feature = "client")]
pub mod graphics;
pub mod networking;
pub mod model;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

use std::net::ToSocketAddrs;
use std::{io::Result, net::SocketAddr};
use std::env;
use std::io::{self, Write};

#[cfg(feature = "client")]
use client::game;
use networking::example::both::{echo_server_both, console_client_both};
use networking::example::{echo_server_udp, console_client_udp, console_client_tcp, echo_server_tcp};

pub fn grab_console_line(prompt: &str) -> String {
    let mut buffer = String::new();
    io::stdout().write_all(prompt.as_bytes()).unwrap();
    io::stdout().flush().unwrap();
    io::stdin().read_line(&mut buffer).unwrap();
    String::from(buffer.trim())
}

pub fn auto_addr() -> (SocketAddr, SocketAddr) {
    let auto_addr_str = ("127.0.0.1:1234", "127.0.0.1:1235");
    // let auto_addr_str = ("test.neotrias.link:1234", "test.neotrias.link:1235");
    (
        auto_addr_str.0.to_socket_addrs().unwrap().next().unwrap(),
        auto_addr_str.1.to_socket_addrs().unwrap().next().unwrap()
    )
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    if args.len() == 1 {
        #[cfg(feature = "client")]
        game::Game::run(Some(auto_addr()));
        return Ok(());
    }
    match args[1].as_str() {
        "test" => {
            test::test();
        },
        "client" => {
            #[cfg(feature = "client")]
            game::Game::run(Some(auto_addr()));
        },
        _ => {
            let ports = (1234, 1235);
            // let ports: (u16, u16) =
            //             (grab_console_line("UDP port: ").parse().expect("Invalid port"),
            //              grab_console_line("TCP port: ").parse().expect("Invalid port"));
            let addresses: (SocketAddr, SocketAddr) = (
                format!("127.0.0.1:{}", ports.0).parse().unwrap(),
                format!("127.0.0.1:{}", ports.1).parse().unwrap()
            );
            // let addresses = auto_addr;
            match args[1].as_str() {
                "server" => {
                    #[cfg(feature = "server")]
                    server::main::Server::run(ports)?
                },
                "udpserver" => {
                    echo_server_udp(ports)?
                },
                "udpclient" => {
                    console_client_udp(addresses)?
                },
                "tcpclient" => {
                    console_client_tcp(addresses)?
                },
                "tcpserver" => {
                    echo_server_tcp(ports)?
                },
                "bothserver" => {
                    echo_server_both(ports)?
                },
                "bothclient" => {
                    console_client_both(addresses)?
                },
                _ => {
                    println!("Unknown mode");
                }
            }
        }
    }
    Ok(())
}

pub mod test {
    use std::f32::consts::PI;

    use nalgebra::{Rotation2, Vector2, Vector3};

    use crate::model::{world::{World, character::{CharacterIDGenerator, CharacterID, CharacterIDRange}, system::{caster_minion, auto_attack::AutoAttackCommand, movement::MoveCharacter}, WorldError, commands::{WorldCommand, CharacterCommand}, component::ComponentID}, TICK_RATE};

    pub fn test() {
        let mut wa = World::new();
        let mut wb = World::new();
        let mut id_gen = CharacterIDGenerator::new();
        for _ in 0..5 {
            wa.update(1.0 / TICK_RATE);
            wb.update(1.0 / TICK_RATE);
            wa.diff(&wb).iter().for_each(|ln| println!("{}", ln));
        }
    }

    pub fn make_mover(id: CharacterID, world: &mut World) -> Result<(), WorldError> {
        let p = Vector2::new(-10.0, -10.0);
        let dest = Vector2::new(10.0, 10.0);
        caster_minion::create(world, &id, Vector3::new(p.x, p.y, 0.0))?;
        let mvcmd = WorldCommand::CharacterComponent(
            id,
            ComponentID::Movement,
            CharacterCommand::Movement(MoveCharacter {
                destination: dest,
                reset: true,
            })
        );
        world.run_command(0, mvcmd)?;
        Ok(())
    }

    pub fn make_attack_circle(count: usize, dist: f32, mut id_gen: CharacterIDRange, world: &mut World) -> Result<(), WorldError> {
        let mut ids: Vec<CharacterID> = vec![];
        for i in 0..count {
            let id = id_gen.next_id().ok_or(WorldError::InvalidCommand)?;
            let p: Vector2<f32> = Rotation2::new(i as f32 / count as f32 * 2.0 * PI)
                .transform_vector(&Vector2::new(dist, 0.0));
            caster_minion::create(world, &id, Vector3::new(p.x, p.y, 0.0))?;
            ids.push(id);
        }
        for i in 0..count {
            let curr = ids[i];
            let next = ids[(i + 1) % count];
            let rng = id_gen.take_range(1000);
            let aacmd = WorldCommand::CharacterComponent(
                curr,
                ComponentID::AutoAttack,
                CharacterCommand::AutoAttack(AutoAttackCommand {
                    projectile_gen_ids: rng,
                    target: next,
                })
            );
            world.run_command(0, aacmd)?;
        }
        Ok(())
    }
}
