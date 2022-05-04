
use std::{rc::Rc, cmp};

use nalgebra::{Vector2, Similarity3, Vector3, Isometry3, Translation3};

use crate::graphics::*;
use freetype_sys::{FT_Library, FT_Init_FreeType, FT_Done_FreeType, FT_Face, FT_New_Memory_Face, FT_Set_Pixel_Sizes, FT_Get_Char_Index, FT_UInt, FT_LOAD_DEFAULT, FT_Load_Glyph, FT_GLYPH_FORMAT_BITMAP, FT_Render_Glyph, FT_RENDER_MODE_NORMAL, FT_Glyph_Metrics, FT_Pos};

use self::packing::{GlyphSize, GlyphPacking};

pub fn default_characters() -> Vec<char> {
    (32..127).map(|i| char::from_u32(i).unwrap()).collect()
}

pub struct FontLibrary {
    ft_library: FT_Library,
    shader: Rc<Shader>
}

impl FontLibrary {
    pub fn new() -> FontLibrary {
        unsafe {
            let mut font_library = FontLibrary{
                ft_library: 0usize as _,
                shader: Rc::new(shader())
            };
            let error = FT_Init_FreeType(&mut font_library.ft_library);
            if error != 0 {
                panic!("Error initializing freetype: {}", error);
            }
            font_library
        }
    }
}

// It's ok to drop the FontLibrary before all fonts are dropped because:
// - Rc will ensure the shader isn't dropped
// - The FT_Library is only needed to instantiate new fonts.
//   Since it is not needed for existing fonts, the FT_Library can safely be dropped
//   before all fonts are dropped
impl Drop for FontLibrary {
    fn drop(&mut self) {
        unsafe {
            FT_Done_FreeType(self.ft_library);
        }
    }
}

impl FontLibrary {
    pub fn make_font<'a, T>(&mut self, path: &str, font_size: i32, char_codes: T) -> Font
            where T: Iterator<Item = &'a char> {
        let info = make_font(self, path, font_size, char_codes);
        Font::new(self, &info)
    }
}

struct GlyphBitmap {
    width: FT_Pos,
    height: FT_Pos,
    buffer: Vec<u8>,
    metrics: FT_Glyph_Metrics,
    char: char,
    char_index: FT_UInt
}

impl GlyphSize<FT_UInt> for GlyphBitmap {
    fn id(&self) -> FT_UInt {
        self.char_index
    }
    fn width(&self) -> packing::Coord {
        self.width as packing::Coord
    }
    fn height(&self) -> packing::Coord {
        self.height as packing::Coord
    }
}

mod packing;

// applies packing by copying glyphs to positions specified by the packing into a new vector
fn apply_packing(glyphs: &Vec<GlyphBitmap>, packing: &GlyphPacking<FT_UInt>) -> Vec<u8> {
    let mut image: Vec<u8> = Vec::new();
    let width: usize = packing.width().try_into().unwrap();
    let height: usize = packing.height().try_into().unwrap();
    image.resize(width * height, 0);
    for glyph in glyphs {
        let uncv_l = packing.get_glyph_pos(glyph.char_index).unwrap();
        let location: Vector2<usize> = Vector2::new(
            uncv_l.x.try_into().unwrap(),
            uncv_l.y.try_into().unwrap());
        for y in 0..glyph.height as usize {
            for x in 0..glyph.width as usize {
                image[(location.y + y) * width + location.x + x] =
                    glyph.buffer[y * glyph.width as usize + x];
            }
        }
    }
    image
}

// all the glyph metrics I think you need to render text correctly
// units are pixels
#[derive(Clone)]
pub struct GlyphMetrics {
    pub glyph_pos: Vector2<f32>,
    pub glyph_size: Vector2<f32>,
    pub advance: f32,
    pub lsb: f32, // left side bearing
    pub tsb: f32, // top side bearing
}

pub struct FontInfo {
    pub image_buffer: Vec<u8>,
    pub image_size: Vector2<u32>,
    pub char_data: HashMap<char, GlyphMetrics>,
    pub font_size: f32
}

pub fn make_font<'a, T>(library: &FontLibrary, path: &str, font_size: i32, char_codes: T) -> FontInfo
        where T: Iterator<Item = &'a char> {
    unsafe {
        let mut face: FT_Face = 0usize as _;

        let data = match std::fs::read(path) {
            Result::Ok(data) => data,
            Result::Err(err) => panic!("Failed to read file {}: {}", path, err)
        };

        // load all glyph data from freetype
        let error = FT_New_Memory_Face(
            library.ft_library,
            data.as_ptr(),
            data.len().try_into().unwrap(),
            0,
            &mut face
        );
        if error != 0 {
            panic!("Error loading font ({}): {}", path, error);
        }
        let error = FT_Set_Pixel_Sizes(
            face,
            0,
            font_size.try_into().expect("Invalid negative font size")
        );
        if error != 0 {
            panic!("Error setting font size ({}): {}", path, error);
        }
        let load_flags = FT_LOAD_DEFAULT;
        let glyphs = char_codes.map(|c| {
            // load glyph
            let index = FT_Get_Char_Index(face, ((*c) as u64).try_into().unwrap());
            let error = FT_Load_Glyph(
                face,
                index,
                load_flags
            );
            if error != 0 {
                panic!("Error loading font glyph ({}) at index {}: {}", path, index, error);
            }
            if (*(*face).glyph).format != FT_GLYPH_FORMAT_BITMAP {
                let error = FT_Render_Glyph((*face).glyph, FT_RENDER_MODE_NORMAL);
                if error != 0 {
                    panic!("Error rendering font glyph({}) at index {}: {}", path, index, error);
                }
            }

            // save glyph
            let slot = (*face).glyph;
            let bitmap = &(*slot).bitmap;
            // maybe eventually, instead of this way, we will just FT_LOAD_BITMAP_METRICS_ONLY
            // to compute spritesheet packing and then re-render again, copying directly to spritesheet.
            // i have no idea if that would actually be faster because while memory would be saved and fewer
            // pixels would be copied, glyph metrics would be calculated twice, and i have no idea how costly
            // that would be.
            GlyphBitmap {
                buffer: {
                    let mut buffer = Vec::<u8>::with_capacity((bitmap.width * bitmap.rows).try_into().unwrap());
                    for i in 0..bitmap.rows {
                        let row_start: usize = (i * bitmap.pitch).try_into().unwrap();
                        for i in 0..bitmap.pitch {
                            let pos: usize = row_start + (i as usize);
                            buffer.push(*bitmap.buffer.add(pos));
                        }
                    }
                    buffer
                },
                width: bitmap.width.try_into().unwrap(),
                height: bitmap.rows.try_into().unwrap(),
                metrics: (*slot).metrics,
                char_index: index,
                char: *c
            }
        }).collect();

        // pack the glyphs
        let packing = match packing::do_font_packing(&glyphs) {
            Some(packing) => packing,
            None => panic!("Error loading font {} size {}: could not pack", path, font_size)
        };

        // apparently it's in fractional pixels?
        let frac_pixels = 1.0 / 64.0;
        let font_size = font_size as f32;

        // create an image and isolate important metrics
        FontInfo {
            image_buffer: apply_packing(&glyphs, &packing),
            image_size: Vector2::new(packing.width(), packing.height()),
            char_data: glyphs.iter().map(|glyph| (
                glyph.char,
                GlyphMetrics {
                    glyph_pos: {
                        let v = packing.get_glyph_pos(glyph.char_index).unwrap();
                        Vector2::new(v.x as f32, v.y as f32)
                    },
                    glyph_size: Vector2::new(glyph.width as f32, glyph.height as f32),
                    advance: glyph.metrics.horiAdvance as f32 * frac_pixels,
                    lsb: glyph.metrics.horiBearingX as f32 * frac_pixels,
                    tsb: glyph.metrics.horiBearingY as f32 * frac_pixels
                }
            )).collect(),
            font_size: font_size
        }
    }
}

#[derive(Clone)]
pub struct Vertex {
    position: Vector2<f32>,
    tex_coord: Vector2<f32>
}

impl ToVec<f32> for Vertex {
    fn to_vec(&self) -> Vec<f32> {
        vec![self.position.x, self.position.y, self.tex_coord.x, self.tex_coord.y]
    } 
}

impl Vertex {
    pub fn new(x: f32, y: f32, u: f32, v: f32) -> Vertex {
        Vertex {
            position: Vector2::<f32>::new(x, y),
            tex_coord: Vector2::<f32>::new(u, v)
        }
    }
}

struct Shader {
    handle: GLuint,
    u_color: GLint,
    u_matrix: GLint,
    u_sampler: GLint,
}

fn shader() -> Shader {
    const VERT_SHADER: &str = r#"
        #version 330
        in vec2 pos;
        in vec2 tex;
        out vec2 midtex;
        uniform mat4 matrix;
        void main() {
            gl_Position = matrix * vec4(pos.x, pos.y, 0.0, 1.0);
            midtex = tex;
        }
    "#;
    const FRAG_SHADER: &str = r#"
        #version 330
        uniform vec4 color;
        uniform sampler2D sampler;
        in vec2 midtex;
        out vec4 final_color;
        void main() {
            float v = texture(sampler, midtex).r;
            final_color = color * v;
        }
    "#;
    let shader_program = shader_program(
        &vec![
            make_shader(VERT_SHADER, GL_VERTEX_SHADER),
            make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
        &vec![Attribute::Position, Attribute::Texture]);
    Shader {
        handle: shader_program,
        u_color: get_uniform(shader_program, "color"),
        u_matrix: get_uniform(shader_program, "matrix"),
        u_sampler: get_uniform(shader_program, "sampler")
    }
    // let u_color = get_uniform(shader_program, "color");
    // let u_matrix = get_uniform(shader_program, "matrix");
    // let u_sampler = get_uniform(shader_program, "sampler");
    // Box::new(move |context: &mut Context| {
    //     unsafe {
    //         glUseProgram(shader_program);
    //         glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
    //         glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
    //         glUniform1i(u_sampler, 0);
    //     }
    // })
}

/*
// all the glyph metrics I think you need to render text correctly
// units are pixels
pub struct GlyphMetrics {
    pub glyph_pos: Vector2<f32>,
    pub glyph_size: Vector2<f32>,
    pub advance: f32,
    pub lsb: f32, // left side bearing
    pub tsb: f32, // top side bearing
}

pub struct FontInfo {
    pub image_buffer: Vec<u8>,
    pub image_size: Vector2<i32>,
    pub char_data: HashMap<char, GlyphMetrics>,
    pub font_size: f32,
}
 */

pub struct Font {
    shader: Rc<Shader>,
    vao: VAO,
    image_size: Vector2<u32>,
    char_data: HashMap<char, GlyphMetrics>,
    font_size: f32,
    index_map: HashMap<char, usize>, // maps each character to its index on the font VBO
    texture: GLuint
}

impl Font {
    fn new(library: &FontLibrary, info: &FontInfo) -> Font {
        let image = unsafe {
            let mut texture: GLuint = 0;
            glGenTextures(1, &mut texture);
            glBindTexture(GL_TEXTURE_2D, texture);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE as GLint);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE as GLint);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST as GLint);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST as GLint);
            glTexImage2D(
                GL_TEXTURE_2D,
                0,
                GL_RED as GLint,
                info.image_size.x as i32,
                info.image_size.y as i32,
                0,
                GL_RED,
                GL_UNSIGNED_BYTE,
                info.image_buffer.as_ptr() as *const _);
            texture
        };
        let (vertices, index_map): (Vec<Vertex>, HashMap<char, usize>) = {
            let mut vertices: Vec<Vertex> = Vec::new();
            let mut index_map: HashMap<char, usize> = HashMap::new();
            let mut index_counter = 0;
            for (c, data) in &info.char_data {
                let index = index_counter;
                index_map.insert(*c, index);

                let min_u = data.glyph_pos.x / info.image_size.x as f32;
                let min_v = data.glyph_pos.y / info.image_size.y as f32;
                let max_u = min_u + data.glyph_size.x / info.image_size.x as f32;
                let max_v = min_v + data.glyph_size.y / info.image_size.y as f32;
                let min_x = 0.0;
                let min_y = 0.0;
                let max_x = min_x + data.glyph_size.x;
                let max_y = min_y + data.glyph_size.y;
                let mut v = vec![
                    Vertex::new(min_x, min_y, min_u, min_v),
                    Vertex::new(min_x, max_y, min_u, max_v),
                    Vertex::new(max_x, max_y, max_u, max_v),
                    Vertex::new(max_x, max_y, max_u, max_v),
                    Vertex::new(max_x, min_y, max_u, min_v),
                    Vertex::new(min_x, min_y, min_u, min_v),
                ];
                index_counter += v.len();
                vertices.append(&mut v);
            }
            (vertices, index_map)
        };
        // let vertices = vec![
        //     Vertex::new(0.0, 0.0, 0.0, 0.0),
        //     Vertex::new(0.0, 1.0, 0.0, 1.0),
        //     Vertex::new(1.0, 1.0, 1.0, 1.0),
        //     Vertex::new(1.0, 1.0, 1.0, 1.0),
        //     Vertex::new(1.0, 0.0, 1.0, 0.0),
        //     Vertex::new(0.0, 0.0, 0.0, 0.0),
        // ];

        let vao = VAO::new(
            &vertices,
            vec![(Attribute::Position, 2), (Attribute::Texture, 2)],
            GL_STATIC_DRAW
        );
        let shader = library.shader.clone();
        Font {
            shader: shader,
            vao: vao,
            image_size: info.image_size,
            char_data: info.char_data.clone(),
            font_size: info.font_size,
            index_map: index_map,
            texture: image 
        }
        // Box::new(move |context: &mut Context| {
        //     unsafe {
        //         glActiveTexture(GL_TEXTURE0);
        //         glBindTexture(GL_TEXTURE_2D, image);
        //         glUseProgram(shader.handle);
        //         glUniformMatrix4fv(shader.u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
        //         glUniform4f(shader.u_color, context.color.x, context.color.y, context.color.z, context.color.w);
        //         glUniform1i(shader.u_sampler, 0);
        //     }
        //     let base = context.matrix.clone();
        //     let mut line_width = 0.0;
        //     let line_height = info.font_size; // temp
        //     let mut trans = Matrix4::identity();
        //     let mut scale = Matrix4::identity();
        //     let text = context.text.clone();
        //     for c in text.chars() {
        //         if c == '\n' {
        //             trans *= Translation3::new(-line_width, line_height, 0.0).to_homogeneous();
        //             line_width = 0.0;
        //             continue;
        //         }
        //         match (info.char_data.get(&c), index_map.get(&c)) {
        //             (Some(metrics), Some(index)) => {
        //                 //scale = Matrix4::new_nonuniform_scaling(&Vector3::new(metrics.glyph_size.x, metrics.glyph_size.y, 1.0));
        //                 trans *= Translation3::new(metrics.lsb, -metrics.tsb, 0.0).to_homogeneous();
        //                 context.matrix = base * trans * scale;
        //                 context.range = VertexRange::Range { first: *index as i32, count: 6 };
        //                 use_shader(context); // super shitty - we fix this shit later
        //                 vao.render(VertexRange::Range { first: *index as i32, count: 6 });
        //                 trans *= Translation3::new(-metrics.lsb + metrics.advance, metrics.tsb, 0.0).to_homogeneous();
        //                 line_width += metrics.advance;
        //             },
        //             _ => ()
        //         }
        //     }
        //     context.matrix = base;
        // })
    }

    pub fn render(&self, matrix: &Matrix4<f32>, text: &str, color: &Vector4<f32>) {
        unsafe {
            glActiveTexture(GL_TEXTURE0);
            glBindTexture(GL_TEXTURE_2D, self.texture);
            glUseProgram(self.shader.handle);
            glUniform4f(self.shader.u_color, color.x, color.y, color.z, color.w);
            glUniform1i(self.shader.u_sampler, 0);
        }
        let base = matrix.clone();
        let mut line_width = 0.0;
        let line_height = self.font_size; // temp
        let mut trans = Matrix4::identity();
        let mut scale = Matrix4::identity();
        for c in text.chars() {
            if c == '\n' {
                trans *= Translation3::new(-line_width, line_height, 0.0).to_homogeneous();
                line_width = 0.0;
                continue;
            }
            match (self.char_data.get(&c), self.index_map.get(&c)) {
                (Some(metrics), Some(index)) => {
                    trans *= Translation3::new(metrics.lsb, -metrics.tsb, 0.0).to_homogeneous();
                    let matrix = base * trans * scale;
                    let range = VertexRange::Range { first: *index as i32, count: 6 };
                    unsafe {
                        glUniformMatrix4fv(self.shader.u_matrix, 1, GL_FALSE, matrix.as_slice().as_ptr());
                    }
                    self.vao.render(VertexRange::Range { first: *index as i32, count: 6 });
                    trans *= Translation3::new(-metrics.lsb + metrics.advance, metrics.tsb, 0.0).to_homogeneous();
                    line_width += metrics.advance;
                },
                _ => ()
            }
        }
    }

    pub fn text_width(&self, text: &str) -> f32{
        struct W {cur_adv: f32, longest: f32}
        text.chars().fold(W {cur_adv: 0.0, longest: 0.0}, |sum: W, c| match self.char_data.get(&c) {
            None => sum, // ignore non-characters
            Some(metrics) =>
                match c {
                    '\n' => W {cur_adv: 0.0, longest: sum.longest}, // new line
                    c => W {
                        cur_adv: sum.cur_adv + metrics.advance,
                        longest: f32::max(sum.cur_adv + metrics.lsb + metrics.glyph_size.x, sum.longest)
                        // true size of line is the first argument of longest: last character's advance plus
                        // current character's lsb + width
                    }
                }
        }).longest
    }
}

impl Drop for Font {
    fn drop(&mut self) {
        unsafe {
            glDeleteTextures(1, &self.texture);
        }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            glDeleteProgram(self.handle);
        }
    }
}
