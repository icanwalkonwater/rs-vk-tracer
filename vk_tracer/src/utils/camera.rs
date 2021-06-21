use nalgebra_glm as glm;

pub struct Camera {
    fov: f32,
    view: glm::Mat4,
    projection: glm::Mat4,
}

impl Camera {
    pub fn new_perspective(position: glm::Vec3, look_at: glm::Vec3, aspect: f32, fov: f32) -> Self {
        Self {
            fov,
            view: glm::look_at(&position, &look_at, &glm::vec3(0.0, 1.0, 0.0)),
            projection: corrected_perspective(glm::perspective(aspect, fov, 0.1, 100.0)),
        }
    }

    pub fn aspect(&mut self, aspect: f32) {
        self.projection = corrected_perspective(glm::perspective(aspect, self.fov, 0.1, 100.0));
    }

    #[inline]
    pub fn aspect_auto(&mut self, size: (u32, u32)) {
        self.aspect(size.0 as f32 / size.1 as f32)
    }

    pub fn translate(&mut self, delta: glm::Vec3) {
        self.view = glm::translate(&self.view, &delta);
    }

    pub fn compute_mvp(&self, model: &glm::Mat4) -> glm::Mat4 {
        self.projection * self.view * model
    }
}

fn corrected_perspective(mut p: glm::Mat4) -> glm::Mat4 {
    *p.get_mut((1, 1)).unwrap() *= -1.0;
    p
}
