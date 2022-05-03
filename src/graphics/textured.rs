
use nalgebra::Vector2;

use crate::graphics::*;

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
    u_sampler: GLint
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
            vec4 v = texture(sampler, midtex);
            final_color = color * vec4(v.x, v.y, v.z, 1) * v.w;
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
    // let shader_program = context.shader_program(
    //     &vec![
    //         context.make_shader(VERT_SHADER, GL_VERTEX_SHADER),
    //         context.make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
    //     &vec![Attribute::Position, Attribute::Texture]);
    // let u_color = context.get_uniform(shader_program, "color");
    // let u_matrix = context.get_uniform(shader_program, "matrix");
    // let u_sampler = context.get_uniform(shader_program, "sampler");
    // Box::new(move |context: &mut Context| {
    //     unsafe {
    //         glUseProgram(shader_program);
    //         glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
    //         glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
    //         glUniform1i(u_sampler, 0);
    //     }
    // })
}

pub struct Renderer {
    shader: Shader,
    vao: VAO
}

impl Renderer {
    pub fn new(vertices: &Vec<Vertex>) -> Renderer {
        Renderer {
            shader: shader(),
            vao: VAO::new(
                vertices,
                vec![(Attribute::Position, 2), (Attribute::Texture, 2)],
                GL_STATIC_DRAW),
        }
        // let mut render_vao = context.vao(
        //     vertices,
        //     vec![(Attribute::Position, 2), (Attribute::Texture, 2)],
        //     GL_STATIC_DRAW
        // );
        // let mut use_shader = shader(context);
        // Box::new(move |mut context: &mut Context| {
        //     use_shader(context);
        //     render_vao(context);
        // })
    }

    pub fn new_square() -> Renderer {
        Renderer::new( &[
            Vertex::new(-0.5, -0.5, 0.0, 0.0),
            Vertex::new(0.5, -0.5, 1.0, 0.0),
            Vertex::new(0.5, 0.5, 1.0, 1.0),
            Vertex::new(0.5, 0.5, 1.0, 1.0),
            Vertex::new(-0.5, 0.5, 0.0, 1.0),
            Vertex::new(-0.5, -0.5, 0.0, 0.0)
        ].to_vec())
    }

    pub fn render(&self, matrix: Matrix4<f32>, color: Vector4<f32>, texture: &Texture, range: VertexRange) {
        unsafe {
            glUseProgram(self.shader.handle);
            glUniformMatrix4fv(self.shader.u_matrix, 1, GL_FALSE, matrix.as_slice().as_ptr());
            glUniform4f(self.shader.u_color, color.x, color.y, color.z, color.w);
            glUniform1i(self.shader.u_sampler, 0);
        }
        texture.bind();
        self.vao.render(range)
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            glDeleteProgram(self.shader.handle);
        }
    }
}