use std::collections::{BTreeMap, HashMap};
use std::ops::Bound::{Included, Unbounded};
use nalgebra::{Vector2, Vector4, Similarity3, Vector3, Rotation2};
use crate::model::player::model::PlayerDataView;
use crate::model::world::character::CharacterType;
use crate::model::world::component::CharacterFlip;
use crate::model::world::system::auto_attack::AutoAttackFireEvent;
use crate::{model::world::character::CharacterID, graphics::{self, TextureOptions}};
use super::camera::CameraMatrix;
use super::game::Game;

const SLIGHT_DEPTH_SEPARATION: f32 = 0.0001;

struct MapLayer {
    _width: u32,
    _height: u32,
    _data: Vec<f32>,
    data_texture: graphics::Texture,
    texture: graphics::Texture,
}

struct Animation {
    timer: f32,
}

enum StandaloneAnimationType {
    FireballCast,
}

struct StandaloneAnimation {
    timer: f32,
    typ: StandaloneAnimationType,
    position: Vector3<f32>,
    flip: CharacterFlip,
}

pub fn regulate_extract_frame(timer: &mut f32, animation_fps: f32, frame_count: usize) -> usize {
    *timer -= f32::floor(*timer * animation_fps / (frame_count as f32))
        * frame_count as f32 / animation_fps;
    (*timer * animation_fps) as usize
}

pub fn extract_frame_or_die(timer: &mut f32, animation_fps: f32, frame_count: usize) -> Option<usize> {
    if *timer * animation_fps >= frame_count as f32 {
        return None;
    }
    Some((*timer * animation_fps) as usize)
}

pub fn extract_frame(timer: f32, animation_fps: f32, frame_count: usize) -> Option<usize> {
    let frame = (timer * animation_fps) as usize;
    if frame >= frame_count {
        None
    } else {
        Some(frame)
    }
}

pub struct Render {
    _font_library: graphics::text::FontLibrary,
    _texture_library: graphics::TextureLibrary,
    text: BTreeMap<i32, graphics::text::Font>,
    _simple_render: graphics::simple::Renderer,
    texture_render: graphics::textured::Renderer,
    map_render: graphics::map::Renderer,

    character_walk_textures: Vec<graphics::Texture>,
    caster_minion_walk_textures: Vec<graphics::Texture>,
    click_animation_textures: Vec<graphics::Texture>,
    fireball_animation_textures: Vec<graphics::Texture>,
    fireball_ball_grow_animation_textures: Vec<graphics::Texture>,
    fireball_flames_grow_animation_textures: Vec<graphics::Texture>,

    map: Vec<MapLayer>,
    animation_data: HashMap<CharacterID, Animation>,
    animation_fps: f32,
    click_animation_timer: f32,
    standalone_animations: Vec<StandaloneAnimation>
}

impl Render {
    pub fn init() -> Self {
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
        let map_render = graphics::map::Renderer::new_square();

        let character_walk_textures: Vec<graphics::Texture> = (1..=12).map(
            |i| texture_library.make_texture(format!("walk_256/Layer {}.png", i).as_str(), &[])
        ).collect();
        let caster_minion_walk_textures: Vec<graphics::Texture> = (1..=12).map(
            |i| texture_library.make_texture(format!("caster_minion_128/Frame {}.png", i).as_str(), &[])
        ).collect();
        let click_animation_textures: Vec<graphics::Texture> = (1..=27).map(
            |i| texture_library.make_texture(format!("click_128/Frame {}.png", i).as_str(), &[])
        ).collect();
        let fireball_animation_textures: Vec<graphics::Texture> = (1..=7).map(
            |i| texture_library.make_texture(format!("fireball_128/Frame {}.png", i).as_str(), &[graphics::TextureOptions::Bilinear])
        ).collect();
        let fireball_ball_grow_animation_textures: Vec<graphics::Texture> = (1..=9).map(
            |i| texture_library.make_texture(format!("fire_ball_cast_128/B{}.png", i).as_str(), &[graphics::TextureOptions::Bilinear])
        ).collect();
        let fireball_flames_grow_animation_textures: Vec<graphics::Texture> = (1..=9).map(
            |i| texture_library.make_texture(format!("fire_ball_cast_128/F{}.png", i).as_str(), &[graphics::TextureOptions::Bilinear])
        ).collect();

        let map: Vec<MapLayer> = [("map/grass.png", "grass.png"), ("map/water.png", "water.png")].iter()
            .map(|(map_file, texture)| {
            let img_obj = image::io::Reader::open(map_file).unwrap().decode().unwrap();
            let img = img_obj.as_rgba8().unwrap();
            let img_data = img.as_raw();
            let data: Vec<u8> = img_data.iter().skip(3).step_by(4).copied().collect();
            MapLayer {
                _width: img.width(),
                _height: img.height(),
                _data: data.iter().map(|pixel| *pixel as f32 / 255.0).collect(),
                data_texture: texture_library.make_texture_from(img.width(), img.height(), &data, &[TextureOptions::Red, TextureOptions::Bilinear]),
                texture: texture_library.make_texture(texture, &[TextureOptions::Repeating, TextureOptions::Bilinear])
            }
        }).collect();

        let animation_data: HashMap<CharacterID, Animation> = HashMap::new();
        let animation_fps = 12.0;
        let click_animation_timer = 0.0;

        Self {
            _font_library: font_library,
            _texture_library: texture_library,
            text,
            _simple_render: simple_render,
            texture_render,
            map_render,
            character_walk_textures,
            caster_minion_walk_textures,
            click_animation_textures,
            fireball_animation_textures,
            fireball_ball_grow_animation_textures,
            fireball_flames_grow_animation_textures,
            map,
            animation_data,
            animation_fps,
            click_animation_timer,
            standalone_animations: vec![]
        }
    }

    pub fn render(&mut self, game: &mut Game, delta_time: f32) {
        let approx_font_size = game.ui_scale;
        let x = (Included(approx_font_size as i32), Unbounded);
        let game_font = match self.text.range(x).next() {
            Some((_, font)) => font,
            None => {
                self.text.iter().next_back().expect("No fonts loaded").1
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

        // display map
        for MapLayer { _width: _, _height: _, _data: _, data_texture, texture } in self.map.iter() {
            let full_scale = 16.0;
            let tile_count = 8.0;
            let matrix = graphics::make_matrix(Vector2::new(0.0, 0.0), Vector2::new(full_scale, full_scale), 0.0);
            self.map_render.render(&(proj_view * matrix), &Vector4::new(1.0, 1.0, 1.0, 1.0), texture, data_texture, tile_count, graphics::VertexRange::Full);
        }

        for anim in self.standalone_animations.iter_mut() {
            anim.timer += delta_time;
        }

        enum Renderable {
            Click(Vector3<f32>, usize),
            Character(CharacterID),
            CharacterCast(Vector3<f32>, usize),
            StandaloneAnimation(Vector3<f32>, usize, CharacterFlip)
        }
        let mut renderables = vec![];
        renderables.extend(game.world.characters.iter().map(|cid| Renderable::Character(*cid)));
        let render_click = || -> Option<Vector2<f32>> {
            if let Some(dest) = game.destination {
                if let Some(cid) = selected_char {
                    if let Some(movement) = game.world.movement.components.get(&cid) {
                        if let Some(dest2) = movement.destination {
                            if (dest - dest2).magnitude() < 0.001 {
                                game.destination = None;
                            }
                        }
                    }
                }
                return Some(dest)
            }
            if let Some(cid) = selected_char {
                if let Some(movement) = game.world.movement.components.get(&cid) {
                    return movement.destination
                }
            }
            None
        }();
        let render_click_frame = if let Some(destination) = render_click {
            self.click_animation_timer += delta_time;
            let frame = regulate_extract_frame(&mut self.click_animation_timer, self.animation_fps, self.click_animation_textures.len());
            renderables.push(Renderable::Click(Vector3::new(destination.x, destination.y, 0.0), frame));
            let scale = 0.5;
            let matrix = graphics::make_matrix(
                destination + Vector2::new(0.0, -0.18),
                Vector2::new(scale, scale),
                0.0
            );
            self.texture_render.render(
                &(proj_view * matrix),
                &Vector4::new(0.5, 0.5, 0.5, 0.5),
                &self.click_animation_textures[frame],
                graphics::VertexRange::Full
            );
            frame
        } else {
            self.click_animation_timer = 0.0;
            0
        };
        for (cid, auto_attack) in &game.world.auto_attack.components {
            || -> Option<()> {
                let frames = self.fireball_ball_grow_animation_textures.len();
                let animation_length = frames as f32 / self.animation_fps;
                let base = game.world.base.components.get(cid)?;
                let execution = auto_attack.execution.as_ref()?;
                let info = game.world.info.auto_attack.get(&base.ctype)?;
                let position = base.position +
                    Vector3::new(
                        info.projectile_offset.x * base.flip.dir(),
                        info.projectile_offset.y,
                        info.projectile_offset.z) +
                    Vector3::new(0.0, SLIGHT_DEPTH_SEPARATION, 0.0);
                let timer = execution.timer;
                let fire_time = info.fsm.get_event_time(execution.starting_attack_speed, AutoAttackFireEvent)?;
                let start_time = fire_time - animation_length;

                if start_time <= timer && timer < fire_time {
                    let frame = ((timer - start_time) / animation_length * frames as f32) as usize;
                    renderables.push(Renderable::CharacterCast(position + Vector3::new(0.0, SLIGHT_DEPTH_SEPARATION, 0.0), frame));
                }
                if timer - delta_time <= start_time && start_time < timer {
                    // start the particles
                    self.standalone_animations.push(StandaloneAnimation {
                        timer: timer - start_time,
                        typ: StandaloneAnimationType::FireballCast,
                        position,
                        flip: base.flip
                    });
                }
                None
            }();
        }

        let mut dead_anim = vec![];
        for (i, anim) in self.standalone_animations.iter_mut().enumerate() {
            match anim.typ {
                StandaloneAnimationType::FireballCast => {
                    let textures = &self.fireball_flames_grow_animation_textures;
                    if let Some(frame) = extract_frame_or_die(&mut anim.timer, self.animation_fps, textures.len()) {
                        renderables.push(Renderable::StandaloneAnimation(anim.position + Vector3::new(0.0, SLIGHT_DEPTH_SEPARATION, 0.0), frame, anim.flip));
                    } else {
                        dead_anim.push(i);
                    }
                }
            }
        }
        for i in dead_anim.iter().rev() {
            self.standalone_animations.remove(*i);
        }

        renderables.sort_by_key(|elt| match elt {
            // sort by float is cringe
            Renderable::Click(pos, _) |
            Renderable::StandaloneAnimation(pos, _, _) |
            Renderable::CharacterCast(pos, _) =>
                Result::unwrap_or(ordered_float::NotNan::new(pos.y), ordered_float::NotNan::new(f32::MAX).unwrap()),
            Renderable::Character(cid) => {
                if let Some(base) = game.world.base.components.get(cid) {
                    Result::unwrap_or(ordered_float::NotNan::new(base.position.y), ordered_float::NotNan::new(f32::MAX).unwrap())
                } else {
                    ordered_float::NotNan::new(f32::MAX).unwrap()
                }
            },
        });

        // characters
        for renderable in renderables {
            match renderable {
                Renderable::Character(cid) => {
                    let cid = &cid;
                    if let Some(base) = game.world.base.components.get(cid) {
                        match base.ctype {
                            CharacterType::IceWiz | CharacterType::CasterMinion => {
                                (|| -> Option<()> {
                                    let Animation { timer: animation_time } = match self.animation_data.get_mut(cid) {
                                        None => {
                                            self.animation_data.insert(*cid, Animation {
                                                timer: 0.0,
                                            });
                                            self.animation_data.get_mut(cid).unwrap()
                                        },
                                        Some(time) => time,
                                    };
                                    let auto_attack = game.world.auto_attack.components.get(cid)?;
                                    let movement = game.world.movement.components.get(cid)?;
                                    let textures = match base.ctype {
                                        CharacterType::IceWiz => &self.character_walk_textures,
                                        CharacterType::CasterMinion => &self.caster_minion_walk_textures,
                                        // CharacterType::Projectile => &fireball_animation_textures,
                                        _ => return Some(())
                                    };
                                    let frame;
                                    if auto_attack.execution.is_none() &&
                                        (auto_attack.targeting.is_some() ||
                                         movement.destination.is_some()) {
                                        *animation_time += delta_time;
                                        frame = regulate_extract_frame(animation_time, self.animation_fps, textures.len());
                                    } else {
                                        *animation_time = 0.0;
                                        frame = 0;
                                    }
                                    let flip_dir: f32 = match base.flip {
                                        CharacterFlip::Left => -1.0,
                                        CharacterFlip::Right => 1.0
                                    };
                                    let scale = match base.ctype {
                                        CharacterType::IceWiz => 1.0,
                                        CharacterType::CasterMinion => 0.5,
                                        _ => return Some(())
                                    };
                                    let offset = match base.ctype {
                                        CharacterType::IceWiz |
                                        CharacterType::CasterMinion => Vector2::new(0.0, -100.0 / 256.0 * scale),
                                        _ => return Some(())
                                    };
                                    let matrix = graphics::make_matrix(
                                        Vector2::new(base.position.x, base.position.y) + offset,
                                        Vector2::new(flip_dir * scale, scale),
                                        0.0
                                    );
                                    let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
                                    self.texture_render.render(
                                        &(proj_view * matrix),
                                        &color,
                                        &textures[frame],
                                        graphics::VertexRange::Full
                                    );

                                    // render player name below player
                                    if let Some(name) = game.character_name.get(cid) {
                                        let text_width = game_font.text_width(name.as_str());
                                        let player_view_pos = game.camera.world_to_view_pos(Vector2::new(base.position.x, base.position.y));
                                        let offset = Vector2::new(-text_width / 2.0, game_font.line_height());
                                        let sim = Similarity3::<f32>::new(
                                            Vector3::new(player_view_pos.x + offset.x, player_view_pos.y + offset.y, 0.0),
                                            Vector3::z() * std::f32::consts::FRAC_PI_4 * 0.0,
                                            1.0
                                        );
                                        game_font.render(&(proj * sim.to_homogeneous()), name.as_str(), &Vector4::new(1.0, 1.0, 1.0, 1.0));
                                    }
                                    Some(())
                                })();
                            },
                            CharacterType::Projectile => || -> Option<()> {
                                let Animation { timer: animation_time,  } = match self.animation_data.get_mut(cid) {
                                    None => {
                                        self.animation_data.insert(*cid, Animation {
                                            timer: 0.0,
                                        });
                                        self.animation_data.get_mut(cid).unwrap()
                                    },
                                    Some(time) => time,
                                };
                                let textures = &self.fireball_animation_textures;
                                *animation_time += delta_time;
                                let frame = regulate_extract_frame(animation_time, self.animation_fps, textures.len());
                                let position = base.position;
                                let projectile = game.world.projectile.components.get(cid)?;
                                let target_position = game.world.base.components.get(&projectile.target)?.position;
                                // let flip_dir: f32 = match base.flip {
                                //     CharacterFlip::Left => -1.0,
                                //     CharacterFlip::Right => 1.0
                                // };
                                let rotation = Rotation2::rotation_between(
                                    &Vector2::new(1.0, 0.0),
                                    &(
                                        Vector2::new(target_position.x, target_position.y + target_position.z) -
                                        Vector2::new(position.x, position.y + target_position.z)
                                    )
                                ).angle();
                                let scale = 0.5;
                                let offset = Vector2::new(0.0, 0.0);
                                let matrix = graphics::make_matrix(
                                    Vector2::new(base.position.x, base.position.y + base.position.z) + offset,
                                    Vector2::new(scale, scale),
                                    rotation
                                );
                                let color = Vector4::new(1.0, 1.0, 1.0, 1.0);
                                self.texture_render.render(
                                    &(proj_view * matrix),
                                    &color,
                                    &textures[frame],
                                    graphics::VertexRange::Full
                                );
                                Some(())
                            }().unwrap_or(()),
                        };
                    }
                },
                Renderable::Click(position, frame) => {
                    let scale = 0.5;
                    let matrix = graphics::make_matrix(
                        Vector2::new(position.x, position.y + position.z) + Vector2::new(0.0, -0.18),
                        Vector2::new(scale, scale),
                        0.0
                    );
                    self.texture_render.render(
                        &(proj_view * matrix),
                        &Vector4::new(0.5, 0.5, 0.5, 0.5),
                        &self.click_animation_textures[frame],
                        graphics::VertexRange::Full
                    );
                },
                Renderable::CharacterCast(position, frame) => {
                    let scale = 0.5;
                    let matrix = graphics::make_matrix(
                        Vector2::new(position.x, position.y + position.z) + Vector2::new(0.0, 0.0),
                        Vector2::new(scale, scale),
                        0.0
                    );
                    self.texture_render.render(
                        &(proj_view * matrix),
                        &Vector4::new(1.0, 1.0, 1.0, 1.0),
                        &self.fireball_ball_grow_animation_textures[frame],
                        graphics::VertexRange::Full
                    );
                },
                Renderable::StandaloneAnimation(position, frame, flip) => {
                    let scale = 0.5;
                    let matrix = graphics::make_matrix(
                        Vector2::new(position.x, position.y + position.z),
                        Vector2::new(scale * flip.dir(), scale),
                        0.0
                    );
                    self.texture_render.render(
                        &(proj_view * matrix),
                        &Vector4::new(1.0, 1.0, 1.0, 1.0),
                        &self.fireball_flames_grow_animation_textures[frame],
                        graphics::VertexRange::Full
                    );
                }
            }
        }

        if let Some(destination) = render_click {
            let scale = 0.5;
            let matrix = graphics::make_matrix(
                destination + Vector2::new(0.0, -0.18),
                Vector2::new(scale, scale),
                0.0
            );
            self.texture_render.render(
                &(proj_view * matrix),
                &Vector4::new(0.5, 0.5, 0.5, 0.5),
                &self.click_animation_textures[render_click_frame],
                graphics::VertexRange::Full
            );
        }
    }
}

impl Default for Render {
    fn default() -> Self {
        Self::init()
    }
}
