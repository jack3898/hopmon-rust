//! `JSprite` / `JNumber`: DirectDraw blits of vertically stacked frames.

use crate::assets::Assets;
use crate::gfx::Gfx;
use crate::math::round;

#[derive(Clone, Debug)]
pub struct Sprite {
    pub id: u16,
    pub color_key: bool,
    pub x: f32,
    pub y: f32,
    pub width: i32,
    pub height: i32,
    pub draw_w: i32,
    pub draw_h: i32,
    pub frame: i32,
    pub frames: i32,
}

impl Sprite {
    pub fn new(gfx: &mut Gfx, assets: &Assets, id: u16, frames: i32, color_key: bool) -> Sprite {
        let (w, h) = gfx.surface_size(assets, id, color_key);
        Sprite {
            id,
            color_key,
            x: 0.0,
            y: 0.0,
            width: w,
            height: h / frames.max(1),
            draw_w: -1,
            draw_h: -1,
            frame: 0,
            frames,
        }
    }

    pub fn pos_x(&self) -> i32 {
        round(self.x)
    }
    pub fn pos_y(&self) -> i32 {
        round(self.y)
    }
    pub fn set_pos(&mut self, x: i32, y: i32) {
        self.x = x as f32;
        self.y = y as f32;
    }
    pub fn set_center(&mut self, x: i32, y: i32) {
        self.x = (x - self.width / 2) as f32;
        self.y = (y - self.height / 2) as f32;
    }
    fn center_x(&self) -> i32 {
        self.pos_x() + self.width / 2
    }
    fn center_y(&self) -> i32 {
        self.pos_y() + self.height / 2
    }

    pub fn draw(&self, gfx: &mut Gfx, assets: &Assets) {
        let w = if self.draw_w < 0 {
            self.width
        } else {
            self.draw_w
        };
        let h = if self.draw_h < 0 {
            self.height
        } else {
            self.draw_h
        };
        let src = (0, self.frame * self.height, self.width, self.height);
        gfx.blit(
            assets,
            self.id,
            self.color_key,
            src,
            (self.pos_x(), self.pos_y(), w, h),
        );
    }

    /// `DrawScaleXY`: scaled about the sprite centre.
    pub fn draw_scale(&self, gfx: &mut Gfx, assets: &Assets, sx: f32, sy: f32) {
        let w = round(self.width as f32 * sx);
        let h = round(self.height as f32 * sy);
        let x = self.center_x() - w / 2;
        let y = self.center_y() - h / 2;
        let src = (0, self.frame * self.height, self.width, self.height);
        gfx.blit(assets, self.id, self.color_key, src, (x, y, w, h));
    }
}

#[derive(Clone, Debug)]
pub struct Number {
    pub sprite: Sprite,
    pub x: i32,
    pub y: i32,
    pub display_zero: bool,
    pub digits: i32,
    pub value: i32,
}

impl Number {
    pub fn new(gfx: &mut Gfx, assets: &Assets, id: u16, frames: i32) -> Number {
        Number {
            sprite: Sprite::new(gfx, assets, id, frames, true),
            x: 0,
            y: 0,
            display_zero: true,
            digits: 0,
            value: 0,
        }
    }

    pub fn digit_width(&self) -> i32 {
        self.sprite.width / 3
    }

    pub fn width(&self) -> i32 {
        self.digit_width() * self.digits
    }

    pub fn height(&self) -> i32 {
        self.sprite.height
    }

    pub fn draw(&mut self, gfx: &mut Gfx, assets: &Assets) {
        let s = self.value.to_string();
        let len = s.len() as i32;
        let mut x = self.x;
        if self.digits > 0 && len < self.digits {
            for _ in 0..self.digits - len {
                self.sprite.frame = 0;
                self.sprite.set_pos(x, self.y);
                if self.display_zero {
                    self.sprite.draw(gfx, assets);
                }
                x += self.digit_width();
            }
        }
        for (i, ch) in s.bytes().enumerate() {
            if len - i as i32 <= self.digits {
                self.sprite.frame = (ch as i32 - b'0' as i32).clamp(0, self.sprite.frames - 1);
                self.sprite.set_pos(x, self.y);
                self.sprite.draw(gfx, assets);
                x += self.digit_width();
            }
        }
    }
}
