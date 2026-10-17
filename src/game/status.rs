//! `XStatusDisplay`: level number, crystals left and the fire gauge.

use super::Ctx;
use crate::gfx::{SCREEN_H, SCREEN_W};
use crate::sprite::{Number, Sprite};

pub struct Status {
    text_level: Sprite,
    text_crystals: Sprite,
    text_fire_ready: Sprite,
    fire_led: Sprite,
    num_level: Number,
    num_crystals: Number,
}

impl Status {
    pub fn new(ctx: &mut Ctx) -> Status {
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        let mut s = Status {
            text_level: Sprite::new(g, a, 0xb9, 1, true),
            text_crystals: Sprite::new(g, a, 0xbc, 1, true),
            text_fire_ready: Sprite::new(g, a, 0xbd, 2, true),
            fire_led: Sprite::new(g, a, 0xc0, 2, true),
            num_level: Number::new(g, a, 0xba, 10),
            num_crystals: Number::new(g, a, 0xba, 10),
        };
        s.text_level.set_pos(12, 4);
        let w = s.text_level.width;
        s.num_level.x = w + 20;
        s.num_level.y = 5;
        s.num_level.display_zero = true;
        s.num_level.digits = 2;
        let x = w + 44 + s.num_level.digit_width() * 2;
        s.text_crystals.set_pos(x, 4);
        s.num_crystals.x = x + 8 + s.text_crystals.width;
        s.num_crystals.y = 5;
        s.num_crystals.display_zero = true;
        s.num_crystals.digits = 2;
        s
    }

    pub fn draw(&mut self, ctx: &mut Ctx, level: i32, crystals_left: i32, fire_energy: i32) {
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        self.text_level.draw(g, a);
        self.text_crystals.draw(g, a);
        self.num_level.value = level + 1;
        self.num_level.draw(g, a);
        self.num_crystals.value = crystals_left;
        self.num_crystals.draw(g, a);
        let bars = fire_energy / 100;
        self.text_fire_ready.frame = (bars > 9) as i32;
        self.text_fire_ready
            .set_pos(SCREEN_W - 0x11e, SCREEN_H - 0x1f);
        self.text_fire_ready.draw(g, a);
        let mut x = SCREEN_W - 0x117 + self.text_fire_ready.width;
        for i in 0..10 {
            self.fire_led.frame = (i < bars) as i32;
            self.fire_led.set_pos(x, SCREEN_H - 0x1c);
            self.fire_led.draw(g, a);
            x += 1 + self.fire_led.width / 3;
        }
    }
}
