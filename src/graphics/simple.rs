
use nalgebra::Vector2;

use crate::graphics::*;

#[derive(Clone)]
pub struct Vertex {
    position: Vector2<f32>
}

impl ToVec<f32> for Vertex {
    fn to_vec(&self) -> Vec<f32> {
        vec![self.position.x, self.position.y]
    } 
}

impl Vertex {
    pub fn new(x: f32, y: f32) -> Vertex {
        Vertex {
            position: Vector2::<f32>::new(x, y)
        }
    }
}

pub fn shader(context: &mut Context) -> RenderFunction {
    const VERT_SHADER: &str = r#"
        #version 330
        in vec2 pos;
        uniform mat4 matrix;
        void main() {
            gl_Position = matrix * vec4(pos.x, pos.y, 0.0, 1.0);
        }
    "#;
    const FRAG_SHADER: &str = r#"
        #version 330
        uniform vec4 color;
        out vec4 final_color;
        void main() {
            final_color = color;
        }
    "#;
    let shader_program = context.shader_program(
        &vec![
            context.make_shader(VERT_SHADER, GL_VERTEX_SHADER),
            context.make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
        &vec![Attribute::Position]);
    let u_color = context.get_uniform(shader_program, "color");
    let u_matrix = context.get_uniform(shader_program, "matrix");
    Box::new(move |context: &Context| {
        unsafe {
            glUseProgram(shader_program);
            glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
            glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
        }
    })
}

pub fn renderer(context: &mut Context, vertices: &Vec<Vertex>) -> RenderFunction {
    let mut render_vao = context.vao(
        vertices,
        vec![(Attribute::Position, 2)],
        GL_STATIC_DRAW
    );
    let mut use_shader = shader(context);
    Box::new(move |context: &Context| {
        use_shader(&context);
        render_vao(&context);
    })
}

pub fn square_renderer(context: &mut Context) -> RenderFunction {
    renderer(context, &[
        Vertex::new(-0.5, -0.5),
        Vertex::new(0.5, -0.5),
        Vertex::new(0.5, 0.5),
        Vertex::new(0.5, 0.5),
        Vertex::new(-0.5, 0.5),
        Vertex::new(-0.5, -0.5)
    ].to_vec())
}

