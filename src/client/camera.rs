use nalgebra::{Orthographic3, Vector2, Matrix4, Vector3};

pub struct CameraContext {
    pub width: i32,
    pub height: i32,
    pub position: Vector2<f32>,
    pub zoom: f32,
}

pub fn camera_matrix(context: CameraContext) -> Matrix4<f32> {
    let zoom_factor = context.zoom;
    Orthographic3::<f32>::new(
        0.0,
        context.width as f32,
        context.height as f32,
        0.0,
        0.0,
        1.0).to_homogeneous() *
    Matrix4::new_translation(
        &Vector3::new(
            context.position.x,
            context.position.y,
            0.0)) *
    Matrix4::new_nonuniform_scaling(
        &Vector3::new(
            zoom_factor,
            zoom_factor,
            0.0))
}

