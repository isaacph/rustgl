use nalgebra::{Orthographic3, Vector2, Matrix4, Vector3};

pub struct CameraContext {
    pub width: i32,
    pub height: i32,
    pub position: Vector2<f32>, // in world coordinates
    pub zoom: f32, // the number of units (1x1) that can fit in the smaller dimension of the screen
}

pub struct CameraMatrix {
    pub proj: Matrix4<f32>,
    pub view: Matrix4<f32>
}

impl CameraContext {// optimization for later: cache the projection matrix
    pub fn zoom_factor(&self) -> f32 { // will probably be > 1 unless zoom is really really big (like > min(width,height))
        std::cmp::min(self.width, self.height) as f32 / f32::max(0.001, self.zoom)
    }

    pub fn camera_center_offset(&self) -> Vector2<f32> {
        Vector2::new(
            (self.width as f32) / 2.0,
            (self.height as f32) / 2.0
        )
    }

    pub fn matrix(&self) -> CameraMatrix {
        let width = f32::max(1.0, self.width as f32);
        let height = f32::max(1.0, self.height as f32);
        let zoom_factor = self.zoom_factor();
        let camera_center_offset = self.camera_center_offset();
        let proj = Orthographic3::<f32>::new(
            0.0,
            width,
            height,
            0.0,
            0.0,
            1.0).to_homogeneous();
        CameraMatrix {
            proj,
            view:
            Matrix4::new_translation(
                &Vector3::new(
                    camera_center_offset.x,
                    camera_center_offset.y,
                    0.0)) *
            Matrix4::new_nonuniform_scaling(
                &Vector3::new(
                    zoom_factor,
                    zoom_factor,
                    0.0)) *
            Matrix4::new_translation(
                &Vector3::new(
                    -self.position.x,
                    -self.position.y,
                    0.0))
        }
    }

    pub fn view_to_world_pos(&self, position: Vector2<f32>) -> Vector2<f32> {
        (position - self.camera_center_offset()) / self.zoom_factor() + self.position
    }

    pub fn view_to_world_pos_3d(&self, position: Vector3<f32>) -> Vector2<f32> {
        (Vector2::new(position.x, position.y + position.z) - self.camera_center_offset()) / self.zoom_factor() + self.position
    }
    
    pub fn view_to_world_scale(&self, scale: Vector2<f32>) -> Vector2<f32> {
        scale / self.zoom_factor()
    }

    pub fn world_to_view_pos(&self, position: Vector2<f32>) -> Vector2<f32> {
        (position - self.position) * self.zoom_factor() + self.camera_center_offset()
    }
    
    pub fn world_to_view_scale(&self, scale: Vector2<f32>) -> Vector2<f32> {
        scale * self.zoom_factor()
    }
}
