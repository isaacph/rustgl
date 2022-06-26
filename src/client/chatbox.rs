
use nalgebra::{Matrix4, Vector3, Vector4};

use crate::graphics::text::*;
use crate::graphics::*;
pub struct Chatbox<'a> {
    font: &'a Font,
    simple_render: &'a simple::Renderer,
    visible_lines: i32,
    history_length: i32,
    typing: String,
    history: Vec<String>,
    width: f32,
    height: f32,
    flicker_timer: f32,
    typing_flicker: bool,
    fade_timer: f32
}

pub const BAR_FLICKER_TIME: f32 = 0.6;
pub const FADE_START_TIME: f32 = 3.0;
pub const FADE_TIME: f32 = 1.0;

impl Chatbox<'_> {
    pub fn new<'a>(font: &'a Font, simple_render: &'a simple::Renderer, visible_lines: i32, history_length: i32, width: f32) -> Chatbox<'a> {
        assert!(visible_lines >= 0 && history_length >= 0 && width >= 0.0);
        Chatbox::<'a> {
            font,
            simple_render,
            visible_lines,
            history_length,
            typing: String::new(),
            history: Vec::new(),
            width,
            height: (visible_lines + 1) as f32 * font.line_height(),
            flicker_timer: 0.0,
            typing_flicker: false,
            fade_timer: 0.0
        }
    }

    pub fn println(&mut self, line: &str) {
        println!("{}", line);
        let mut lines: Vec<String> = self.font.split_lines(line, Some(self.width));
        let add_len = std::cmp::min(self.history_length as usize, lines.len()) as i32;
        lines.drain(0..(std::cmp::max(0, lines.len() as i32 - add_len)) as usize);
        let history_remove = 
            std::cmp::max(0, self.history.len() as i32 - (self.history_length - add_len)) as usize;
        self.history.drain(0..history_remove);
        self.history.append(&mut lines);
        self.fade_timer = 0.0;
    }

    fn get_visible_history_empty_lines(&self) -> i32 {
        std::cmp::max(0, self.visible_lines - self.history.len() as i32)
    }

    pub fn get_visible_history(&self) -> Vec<&str> {
        let mut vec = Vec::new();
        for i in (std::cmp::max(0, self.history.len() as i32 - self.visible_lines) as usize)..self.history.len() {
            vec.push(self.history[i].as_str());
        }
        vec
    }

    pub fn get_typing(&self) -> &String {
        &self.typing
    }

    pub fn add_typing(&mut self, c: char) {
        self.typing.push(c);
    }

    pub fn remove_typing(&mut self, count: i32) {
        assert!(count >= 0);
        self.typing.truncate(std::cmp::max(0, self.typing.len() as i32 - count) as usize);
    }

    pub fn erase_typing(&mut self) {
        self.typing.clear();
    }

    pub fn set_typing_flicker(&mut self, typing_flicker: bool) {
        self.typing_flicker = typing_flicker;
        self.flicker_timer = 0.0;
        self.fade_timer = 0.0;
    }

    pub fn render(&mut self, proj: &Matrix4<f32>, delta_time: f32) {
        self.fade_timer += delta_time;
        let is_fade = self.fade_timer > FADE_START_TIME && !self.typing_flicker;
        let mut fade = 1.0;
        if is_fade {
            fade = 1.0 - f32::max(0.0, (self.fade_timer - FADE_START_TIME) / FADE_TIME);
        }

        let color = Vector4::new(1.0, 1.0, 1.0, 1.0) * fade;
        let background_color = Vector4::new(0.0, 0.0, 0.0, 0.6) * fade;

        let background_matrix = Matrix4::identity()
            .prepend_translation(&Vector3::new(self.width / 2.0, self.height / 2.0, 0.0))
            .prepend_nonuniform_scaling(&Vector3::new(self.width, self.height, 0.0));
        self.simple_render.render(&(proj * background_matrix), &background_color, VertexRange::Full);
        
        let matrix = Matrix4::identity().append_translation(
            &Vector3::new(
                0.0,
                self.font.ascent() + (self.get_visible_history_empty_lines()) as f32 * self.font.line_height(),
                0.0));
        let matrix = self.get_visible_history().iter().fold(matrix, |matrix, line| {
            self.font.render(&(proj * matrix), line, &color);
            matrix.append_translation(&Vector3::new(0.0, self.font.line_height(), 0.0))
        });

        if self.typing_flicker {
            self.flicker_timer += delta_time;
            while self.flicker_timer > BAR_FLICKER_TIME {
                self.flicker_timer -= BAR_FLICKER_TIME;
            }
        }
        let typing_line = if self.flicker_timer > BAR_FLICKER_TIME / 2.0 && self.typing_flicker {
            self.typing.to_owned() + "|"
        } else {
            self.typing.to_owned()
        };
        self.font.render(&(proj * matrix), typing_line.as_str(), &color);
    }
}