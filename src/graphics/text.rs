
use nalgebra::Vector2;

use crate::graphics::*;
use freetype_sys::{FT_Library, FT_Init_FreeType, FT_Done_FreeType, FT_Face, FT_New_Memory_Face, FT_Set_Pixel_Sizes, FT_Get_Char_Index, FT_UInt, FT_LOAD_DEFAULT, FT_Load_Glyph, FT_GLYPH_FORMAT_BITMAP, FT_Render_Glyph, FT_RENDER_MODE_NORMAL, FT_Glyph_Metrics, FT_Pos};

use self::packing::{GlyphSize, GlyphPacking};

pub fn default_characters() -> Vec<char> {
    (32..127).map(|i| char::from_u32(i).unwrap()).collect()
}

pub struct FontLibrary {
    ft_library: FT_Library
}

impl FontLibrary {
    pub fn init() -> FontLibrary {
        unsafe {
            let mut font_library = FontLibrary{
                ft_library: 0usize as _
            };
            let error = FT_Init_FreeType(&mut font_library.ft_library);
            if error != 0 {
                panic!("Error initializing freetype: {}", error);
            }
            font_library
        }
    }
}

impl Drop for FontLibrary {
    fn drop(&mut self) {
        unsafe {
            FT_Done_FreeType(self.ft_library);
        }
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
pub struct GlyphMetrics {
    pub glyph_pos: Vector2<f32>,
    pub glyph_size: Vector2<f32>,
    pub advance: f32,
    pub lsb: f32
}

pub struct FontInfo {
    pub image: Vec<u8>,
    pub char_data: HashMap<char, GlyphMetrics>
}

pub fn make_font<'a, T>(context: &mut Context, path: &str, font_size: i32, char_codes: T) -> FontInfo
        where T: Iterator<Item = &'a char> {
    let library = &mut context.persistent_objects.font_library;
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
            let index = FT_Get_Char_Index(face, (*c) as u64);
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

        // create an image and isolate important metrics
        FontInfo {
            image: apply_packing(&glyphs, &packing),
            char_data: glyphs.iter().map(|glyph| (
                glyph.char,
                GlyphMetrics {
                    glyph_pos: {
                        let v = packing.get_glyph_pos(glyph.char_index).unwrap();
                        Vector2::new(v.x as f32, v.y as f32)
                    },
                    glyph_size: Vector2::new(glyph.width as f32, glyph.height as f32),
                    advance: glyph.metrics.horiAdvance as f32,
                    lsb: glyph.metrics.horiBearingX as f32
                }
            )).collect()
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

pub fn shader(context: &mut Context) -> RenderFunction {
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
            vec4 v = texture(sampler, midtex);
            final_color = color * vec4(v.x, v.y, v.z, 1) * v.w;
        }
    "#;
    let shader_program = context.shader_program(
        &vec![
            context.make_shader(VERT_SHADER, GL_VERTEX_SHADER),
            context.make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
        &vec![Attribute::Position, Attribute::Texture]);
    let u_color = context.get_uniform(shader_program, "color");
    let u_matrix = context.get_uniform(shader_program, "matrix");
    let u_sampler = context.get_uniform(shader_program, "sampler");
    Box::new(move |context: &Context| {
        unsafe {
            glUseProgram(shader_program);
            glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
            glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
            glUniform1i(u_sampler, 0);
        }
    })
}

pub fn renderer(context: &mut Context, vertices: &Vec<Vertex>) -> RenderFunction {
    let mut render_vao = context.vao(
        vertices,
        vec![(Attribute::Position, 2), (Attribute::Texture, 2)],
        GL_STATIC_DRAW
    );
    let mut use_shader = shader(context);
    Box::new(move |context: &Context| {
        use_shader(&context);
        render_vao(&context);
    })
}
