use nalgebra::{Vector2, Vector3};

pub trait ItClosestRef<'a> {
    fn closest_to(self, to: &Vector2<f32>) -> Option<&'a Vector2<f32>>;
}
pub trait ItClosest {
    fn closest_to(self, to: &Vector2<f32>) -> Option<Vector2<f32>>;
}
pub trait GroundPos {
    fn ground_pos(&self) -> Vector2<f32>;
}

impl<'a, T> ItClosestRef<'a> for T
    where T: Iterator<Item = &'a Vector2<f32>> {
    fn closest_to(self, to: &Vector2<f32>) -> Option<&'a Vector2<f32>> {
        self.fold((None, f32::MAX), |(closest, closest_dist): (Option<&Vector2<f32>>, f32), next| {
            let dir = next - to;
            if dir.x == 0.0 && dir.y == 0.0 {
                return (closest, closest_dist)
            }
            let dist = dir.magnitude();
            if let Some(closest) = closest {
                if dist < closest_dist || dist == closest_dist &&
                        (next.x < closest.x || next.x == closest.x && next.y < closest.y) {
                    return (Some(next), dist)
                }
                (Some(closest), closest_dist)
            } else { (Some(next), dist) }
        }).0
    }
}

impl<T> ItClosest for T
    where T: Iterator<Item = Vector2<f32>> {
    fn closest_to(self, to: &Vector2<f32>) -> Option<Vector2<f32>> {
        self.fold((None, f32::MAX), |(closest, closest_dist): (Option<Vector2<f32>>, f32), next| {
            let dir = next - to;
            if dir.x == 0.0 && dir.y == 0.0 {
                return (closest, closest_dist)
            }
            let dist = dir.magnitude();
            if let Some(closest) = closest {
                if dist < closest_dist || dist == closest_dist &&
                        (next.x < closest.x || next.x == closest.x && next.y < closest.y) {
                    return (Some(next), dist)
                }
                (Some(closest), closest_dist)
            } else { (Some(next), dist) }
        }).0
    }
}

impl GroundPos for Vector3<f32> {
    fn ground_pos(&self) -> Vector2<f32> {
        Vector2::new(self.x, self.y)
    }
}
