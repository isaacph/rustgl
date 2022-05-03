
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

struct Shader {
    handle: GLuint,
    u_color: GLint,
    u_matrix: GLint
}

fn shader() -> Shader {
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
    let shader_program = shader_program(
        &vec![
            make_shader(VERT_SHADER, GL_VERTEX_SHADER),
            make_shader(FRAG_SHADER, GL_FRAGMENT_SHADER)],
        &vec![Attribute::Position]);
    let u_color = get_uniform(shader_program, "color");
    let u_matrix = get_uniform(shader_program, "matrix");
    Shader { handle: shader_program, u_color: u_color, u_matrix: u_matrix }
    // Box::new(move |context: &mut Context| {
    //     unsafe {
    //         glUseProgram(shader_program);
    //         glUniformMatrix4fv(u_matrix, 1, GL_FALSE, context.matrix.as_slice().as_ptr());
    //         glUniform4f(u_color, context.color.x, context.color.y, context.color.z, context.color.w);
    //     }
    // })
}

pub struct Renderer {
    shader: Shader,
    vao: VAO
}

impl Renderer {
    pub fn new(vertices: &Vec<Vertex>) -> Renderer {
        let vao = VAO::new(
            vertices,
            vec![(Attribute::Position, 2)],
            GL_STATIC_DRAW
        );
        Renderer {
            vao: vao,
            shader: shader()
        }
        // let vao = vao: context.vao(
        //     vertices,
        //     vec![(Attribute::Position, 2)],
        //     GL_STATIC_DRAW
        // );
        // let shader = shader(context);
        // Box::new(move |context: &mut Context| {
        //     use_shader(context);
        //     render_vao(context);
        // })
    }
    
    pub fn new_square() -> Renderer {
        Renderer::new(&[
            Vertex::new(-0.5, -0.5),
            Vertex::new(0.5, -0.5),
            Vertex::new(0.5, 0.5),
            Vertex::new(0.5, 0.5),
            Vertex::new(-0.5, 0.5),
            Vertex::new(-0.5, -0.5)
        ].to_vec())
    }

    pub fn render(&self, matrix: Matrix4<f32>, color: Vector4<f32>, range: VertexRange) {
        unsafe {
            glUseProgram(self.shader.handle);
            glUniformMatrix4fv(self.shader.u_matrix, 1, GL_FALSE, matrix.as_slice().as_ptr());
            glUniform4f(self.shader.u_color, color.x, color.y, color.z, color.w);
        }
        self.vao.render(range);
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            glDeleteProgram(self.shader.handle);
        }
    }
}

