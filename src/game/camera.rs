//! `XCamera`: orbiting camera following Hopmon.

use crate::math::{Mat, PI, Vec3, v3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CamState {
    Non,
    Wait,
    RotateLeft,
    RotateRight,
    LevelCompleted,
    GameOver,
}

pub struct Camera {
    pub state: CamState,
    pub local_time: u32,
    pub look_at: Vec3,
    pub ref_pos: Vec3,
    pub distance: f32,
    pub vertical: f32,
    pub horizontal: f32,
    pub view_dir: i32,
    pub view: Mat,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            state: CamState::Non,
            local_time: 0,
            look_at: Vec3::ZERO,
            ref_pos: Vec3::ZERO,
            distance: 0.0,
            vertical: 0.0,
            horizontal: 0.0,
            view_dir: 0,
            view: Mat::IDENTITY,
        }
    }
}

const ROT_STEP: f32 = 0.026_179_9;
const ANGLE_STEP: f32 = 0.008_726_6;
const MAX_VERTICAL: f32 = 1.420_796_4;

impl Camera {
    pub fn reset_for_game(&mut self, pac_pos: Vec3) {
        self.distance = 420.0;
        self.vertical = std::f32::consts::FRAC_PI_4;
        self.horizontal = PI;
        self.view_dir = 0;
        self.look_at = v3(pac_pos.x, 0.0, pac_pos.z);
        self.ref_pos = self.look_at;
        self.update_view();
        self.state = CamState::Wait;
        self.local_time = 0;
    }

    pub fn reset_for_title(&mut self) {
        self.distance = 500.0;
        self.vertical = 0.0;
        self.horizontal = PI;
        self.view_dir = 0;
        self.look_at = Vec3::ZERO;
        self.ref_pos = Vec3::ZERO;
        self.update_view();
        self.state = CamState::Wait;
        self.local_time = 0;
    }

    pub fn start_rotating_left(&mut self) {
        self.view_dir = (self.view_dir + 3) & 3;
        self.state = CamState::RotateLeft;
        self.local_time = 0;
    }

    pub fn start_rotating_right(&mut self) {
        self.view_dir = (self.view_dir + 1) & 3;
        self.state = CamState::RotateRight;
        self.local_time = 0;
    }

    pub fn start_level_completed(&mut self) {
        self.state = CamState::LevelCompleted;
        self.local_time = 0;
    }

    pub fn start_game_over(&mut self) {
        self.state = CamState::GameOver;
        self.local_time = 0;
    }

    pub fn incr_vertical(&mut self, ticks: u32) {
        self.vertical = (self.vertical + ticks as f32 * ANGLE_STEP).min(MAX_VERTICAL);
    }

    pub fn decr_vertical(&mut self, ticks: u32) {
        self.vertical -= ticks as f32 * ANGLE_STEP;
        if self.vertical < 0.2 {
            self.vertical = 0.2;
        }
    }

    pub fn process_frame(&mut self, pac_pos: Vec3) {
        self.local_time += 1;
        match self.state {
            CamState::Wait => {
                self.look_at = v3(pac_pos.x, 0.0, pac_pos.z);
                self.ref_pos = self.look_at;
                self.update_view();
            }
            CamState::RotateLeft | CamState::RotateRight => {
                let step = if self.state == CamState::RotateLeft {
                    -ROT_STEP
                } else {
                    ROT_STEP
                };
                self.horizontal += step;
                self.look_at = v3(pac_pos.x, 0.0, pac_pos.z);
                self.ref_pos = self.look_at;
                self.update_view();
                if self.local_time > 59 {
                    self.state = CamState::Wait;
                    self.local_time = 0;
                }
            }
            CamState::LevelCompleted => {
                self.look_at.y += 0.7;
                self.ref_pos.y += 0.7;
                self.distance -= 0.5;
                if self.distance < 140.0 {
                    self.distance = 140.0;
                }
                self.vertical = (self.vertical + 0.001_221_7).min(MAX_VERTICAL);
                self.horizontal += -ANGLE_STEP;
                self.update_view();
            }
            CamState::GameOver => {
                self.distance += 0.5;
                self.vertical = (self.vertical + 0.001_221_7).min(MAX_VERTICAL);
                self.horizontal += -ANGLE_STEP;
                self.update_view();
            }
            CamState::Non => {}
        }
    }

    pub fn update_view(&mut self) {
        let h = self.vertical.sin() * self.distance;
        let d = self.vertical.cos() * self.distance;
        let eye = v3(
            self.horizontal.sin() * d + self.ref_pos.x,
            self.ref_pos.y + h,
            self.horizontal.cos() * d + self.ref_pos.z,
        );
        self.view = Mat::look_at(eye, self.look_at, v3(0.0, 1.0, 0.0));
    }
}
