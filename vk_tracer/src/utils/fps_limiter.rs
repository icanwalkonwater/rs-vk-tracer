use log::info;
use std::time::{Duration, Instant};

pub struct FpsLimiter {
    last_frame_time: Instant,
    target_frame_time: Duration,
    last_fps_check: Instant,
    frames: f32,
}

impl FpsLimiter {
    #[inline]
    pub fn new(target_fps: f32) -> Self {
        let now = Instant::now();
        Self {
            last_frame_time: now,
            target_frame_time: Duration::from_secs_f32(1.0 / target_fps),
            last_fps_check: now,
            frames: 0.0,
        }
    }

    #[inline]
    pub fn should_render(&self) -> bool {
        self.last_frame_time.elapsed() >= self.target_frame_time
    }

    #[inline]
    pub fn new_frame(&mut self) {
        self.frames += 1.0;

        let elapsed = self.last_fps_check.elapsed().as_secs_f32();
        if elapsed >= 1.0 {
            info!("FPS: {}", self.frames / elapsed);
            self.last_fps_check = Instant::now();
            self.frames = 0.0;
        }

        self.last_frame_time = Instant::now();
    }
}
