//! Non-gameplay scenes: `XLogoObject`, `XTitleObject`, `XSelectLevelObject`
//! (with `XPreviewMap`) and `XEndingObject`, plus `XTitleBackground`.

use super::camera::Camera;
use super::map::{Map, world_texture};
use super::{AppState, Ctx, PState, Process, Scene, map_res_id};
use crate::audio::sfx;
use crate::gfx::{SCREEN_H, SCREEN_W};
use crate::input::dik;
use crate::math::{Axis, HALF_PI, PI};
use crate::obj3d::{Filter, Obj3D, ObjX};
use crate::sprite::{Number, Sprite};

/// Scrolling floor/ceiling planes behind the menus.
pub struct TitleBackground {
    upper: Obj3D,
    lower: Obj3D,
    offset: f32,
}

impl TitleBackground {
    pub fn new() -> TitleBackground {
        let mut upper = Obj3D::rect(720.0, 400.0);
        upper.rotate_by(Axis::X, PI);
        upper.base.pos.y += 120.0;
        upper.filter = Filter::Point;
        let mut lower = Obj3D::rect(720.0, 400.0);
        lower.base.pos.y -= 120.0;
        lower.filter = Filter::Linear;
        TitleBackground {
            upper,
            lower,
            offset: 0.0,
        }
    }

    pub fn process_frame(&mut self, ticks: u32) {
        self.offset += ticks as f32 * 0.02;
    }

    pub fn render(&mut self, gfx: &mut crate::gfx::Gfx) {
        let o = self.offset;
        self.upper.set_rect_texture(0xc1, 0.0, o, 18.0, o + 10.0);
        self.upper.render(gfx);
        self.lower.set_rect_texture(0xc1, 0.0, -o, 18.0, 10.0 - o);
        self.lower.render(gfx);
    }
}

// ---------------------------------------------------------------------------

pub struct Logo {
    process: Process,
    camera: Camera,
    logo: Sprite,
    back: Obj3D,
}

impl Logo {
    pub fn new(ctx: &mut Ctx) -> Logo {
        ctx.audio.stop_music();
        let mut camera = Camera::default();
        camera.reset_for_title();
        let logo = Sprite::new(ctx.gfx, ctx.assets, 0x13c, 1, false);
        let mut back = Obj3D::rect(600.0, 600.0);
        back.rotate_by(Axis::X, -HALF_PI);
        back.set_rect_texture(0x14c, 0.0, 0.0, 1.0, 1.0);
        let mut process = Process::new(ctx);
        process.start_process(0);
        Logo {
            process,
            camera,
            logo,
            back,
        }
    }
}

impl Scene for Logo {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32) {
        if self.process.state == PState::Non {
            return;
        }
        ctx.input.update();
        // Any key skips the logo (not in the original, which always waits).
        let skip = ctx.input.button_pressed() || ctx.input.escape_pressed();
        for _ in 0..ticks {
            self.process.tick += 1;
            if self.process.tick == 50 {
                ctx.audio.play(sfx::LOGO, 100);
            }
            if self.process.tick > 949 || (skip && self.process.tick > 60) {
                ctx.app.set_state(AppState::StartTitle);
                self.process.state = PState::Non;
                return;
            }
        }
    }

    fn render(&mut self, ctx: &mut Ctx) {
        if self.process.state == PState::Non {
            return;
        }
        ctx.gfx.set_3d(self.camera.view);
        self.back.render(ctx.gfx);
        ctx.gfx.set_2d();
        self.logo.set_center(SCREEN_W / 2, SCREEN_H / 2);
        self.logo.draw_scale(ctx.gfx, ctx.assets, 1.0, 1.0);
    }
}

// ---------------------------------------------------------------------------

/// Draws the blinking arrow cursors around a menu item.
fn draw_cursor(ctx: &mut Ctx, item: &Sprite, left: &mut Sprite, right: &mut Sprite) {
    let (x, w, y) = (item.pos_x(), item.width, item.pos_y());
    left.set_pos(x - left.width / 2 - 4, y + 2);
    left.draw(ctx.gfx, ctx.assets);
    right.set_pos(x + 4 + w, y + 2);
    right.draw(ctx.gfx, ctx.assets);
}

struct MenuInput {
    up: bool,
    right: bool,
    down: bool,
    left: bool,
    button: bool,
    escape: bool,
}

fn menu_input(ctx: &mut Ctx) -> MenuInput {
    let i = &mut *ctx.input;
    i.update();
    MenuInput {
        up: i.key_pressed(dik::UP) || i.joy_dir_pressed(0),
        right: i.key_pressed(dik::RIGHT) || i.joy_dir_pressed(1),
        down: i.key_pressed(dik::DOWN) || i.joy_dir_pressed(2),
        left: i.key_pressed(dik::LEFT) || i.joy_dir_pressed(3),
        button: i.button_pressed(),
        escape: i.escape_pressed(),
    }
}

pub struct Title {
    process: Process,
    camera: Camera,
    pac: ObjX,
    background: TitleBackground,
    cursor_left: Sprite,
    cursor_right: Sprite,
    menu: [Sprite; 2],
    text_hopmon: Sprite,
    text_copyright: Sprite,
    cursor: i32,
}

impl Title {
    pub fn new(ctx: &mut Ctx) -> Title {
        ctx.audio.play_music(0);
        let mut camera = Camera::default();
        camera.reset_for_title();
        let mut pac = ObjX::new(ctx.assets, Some(0x94));
        pac.rotate_by(Axis::X, -HALF_PI);
        pac.scale_by(170.0);
        let process = Process::new(ctx);
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        let mut t = Title {
            process,
            camera,
            pac,
            background: TitleBackground::new(),
            cursor_left: Sprite::new(g, a, 0xd0, 2, true),
            cursor_right: Sprite::new(g, a, 0xd1, 2, true),
            menu: [
                Sprite::new(g, a, 0xcd, 2, true),
                Sprite::new(g, a, 0xc9, 2, true),
            ],
            text_hopmon: Sprite::new(g, a, 0xdb, 1, true),
            text_copyright: Sprite::new(g, a, 0xf8, 1, true),
            cursor: -1,
        };
        t.process.start_fade_in(0);
        t
    }
}

impl Scene for Title {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32) {
        if self.process.state == PState::Non {
            return;
        }
        self.process.tick += ticks;
        match self.process.state {
            PState::FadeIn => {
                self.process.process_fade_in();
                if self.process.tick > 119 {
                    self.cursor = 0;
                    self.process.start_process(0);
                }
            }
            PState::FadeOut => {
                self.process.process_fade_out();
                if self.process.tick > 119 {
                    let next = if self.process.process_type == 0 {
                        AppState::StartSelLevel
                    } else {
                        AppState::Exit
                    };
                    ctx.app.set_state(next);
                    self.process.state = PState::Non;
                    return;
                }
            }
            _ => {}
        }
        self.background.process_frame(ticks);
        self.pac.rotate_by(Axis::Y, ticks as f32 * 0.010_472);
        if self.process.state != PState::Process {
            return;
        }
        let blink = (self.process.tick % 50 > 24) as i32;
        self.cursor_left.frame = blink;
        self.cursor_right.frame = blink;
        let mi = menu_input(ctx);
        let mut select = mi.button;
        if mi.escape {
            self.cursor = 1;
            select = true;
        }
        if mi.up {
            ctx.audio.play(sfx::MENU_MOVE, 100);
            self.cursor -= 1;
            if self.cursor < 0 {
                self.cursor = 1;
            }
        } else if mi.down {
            ctx.audio.play(sfx::MENU_MOVE, 100);
            self.cursor += 1;
            if self.cursor > 1 {
                self.cursor = 0;
            }
        } else if select {
            ctx.audio.play(sfx::MENU_SELECT, 100);
            self.process
                .start_fade_out(if self.cursor == 0 { 0 } else { 1 });
        }
    }

    fn render(&mut self, ctx: &mut Ctx) {
        if self.process.state == PState::Non {
            return;
        }
        ctx.gfx.set_3d(self.camera.view);
        self.background.render(ctx.gfx);
        self.pac.render(ctx.gfx);
        ctx.gfx.set_2d();
        let (cx, cy) = (SCREEN_W / 2, SCREEN_H / 2);
        self.text_hopmon.set_center(cx, cy - 0xa0);
        self.text_hopmon.draw(ctx.gfx, ctx.assets);
        self.text_copyright.set_center(cx, cy - 0x5a);
        self.text_copyright.draw(ctx.gfx, ctx.assets);
        let mut y = cy + 0x96;
        for (i, m) in self.menu.iter_mut().enumerate() {
            m.set_center(cx, y);
            m.frame = (self.cursor == i as i32) as i32;
            m.draw(ctx.gfx, ctx.assets);
            y += 8 + m.height;
        }
        if self.process.state == PState::Process && self.cursor >= 0 {
            let item = self.menu[self.cursor as usize].clone();
            draw_cursor(ctx, &item, &mut self.cursor_left, &mut self.cursor_right);
        }
        self.process.draw_fade(ctx.gfx, ctx.assets);
    }
}

// ---------------------------------------------------------------------------

/// `XPreviewMap`: small rotating copy of the selected level.
struct PreviewMap {
    map: Map,
}

impl PreviewMap {
    fn new(ctx: &mut Ctx) -> PreviewMap {
        let lines = ctx.assets.text_lines(map_res_id(ctx.app));
        let (mut map, _) = Map::build(&lines, 10.0, world_texture(ctx.app.world()), true);
        map.mesh.rotate_by(Axis::X, -std::f32::consts::FRAC_PI_6);
        PreviewMap { map }
    }

    fn process_frame(&mut self, ticks: u32) {
        let m = &mut self.map.mesh;
        m.rotate_by(Axis::X, std::f32::consts::FRAC_PI_6);
        m.rotate_by(Axis::Y, ticks as f32 * 0.005_236);
        m.rotate_by(Axis::X, -std::f32::consts::FRAC_PI_6);
    }
}

pub struct SelectLevel {
    process: Process,
    camera: Camera,
    background: TitleBackground,
    map: PreviewMap,
    cursor_left: Sprite,
    cursor_right: Sprite,
    cursor2_left: Sprite,
    cursor2_right: Sprite,
    menu: [Sprite; 3],
    text_level: Sprite,
    level_led: Sprite,
    num_level: Number,
    cursor: i32,
    in_select: bool,
}

impl SelectLevel {
    pub fn new(ctx: &mut Ctx) -> SelectLevel {
        ctx.audio.play_music(0);
        let mut camera = Camera::default();
        camera.reset_for_title();
        let process = Process::new(ctx);
        let map = PreviewMap::new(ctx);
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        let mut num_level = Number::new(g, a, 0xba, 10);
        num_level.display_zero = true;
        num_level.digits = 2;
        let mut s = SelectLevel {
            process,
            camera,
            background: TitleBackground::new(),
            map,
            cursor_left: Sprite::new(g, a, 0xd0, 2, true),
            cursor_right: Sprite::new(g, a, 0xd1, 2, true),
            cursor2_left: Sprite::new(g, a, 0xce, 2, true),
            cursor2_right: Sprite::new(g, a, 0xcf, 2, true),
            menu: [
                Sprite::new(g, a, 0xcd, 2, true),
                Sprite::new(g, a, 0xcc, 2, true),
                Sprite::new(g, a, 200, 2, true),
            ],
            text_level: Sprite::new(g, a, 0xb9, 1, true),
            level_led: Sprite::new(g, a, 0x140, 3, true),
            num_level,
            cursor: -1,
            in_select: false,
        };
        s.process.start_fade_in(0);
        s
    }
}

impl Scene for SelectLevel {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32) {
        if self.process.state == PState::Non {
            return;
        }
        self.process.tick += ticks;
        match self.process.state {
            PState::FadeIn => {
                self.process.process_fade_in();
                if self.process.tick > 119 {
                    self.cursor = 0;
                    self.process.start_process(0);
                }
            }
            PState::FadeOut => {
                self.process.process_fade_out();
                if self.process.tick > 119 {
                    if self.process.process_type == 0 {
                        ctx.app.set_state(AppState::StartGame);
                    } else {
                        ctx.app.current_level = 0;
                        ctx.app.set_state(AppState::StartTitle);
                    }
                    self.process.state = PState::Non;
                    return;
                }
            }
            _ => {}
        }
        self.background.process_frame(ticks);
        self.map.process_frame(ticks);
        if self.process.state != PState::Process {
            return;
        }
        let blink = (self.process.tick % 50 > 24) as i32;
        for s in [
            &mut self.cursor_left,
            &mut self.cursor_right,
            &mut self.cursor2_left,
            &mut self.cursor2_right,
        ] {
            s.frame = blink;
        }
        let mi = menu_input(ctx);
        let mut select = mi.button;
        if !self.in_select {
            if mi.escape {
                self.cursor = 2;
                select = true;
            }
            if mi.up {
                ctx.audio.play(sfx::MENU_MOVE, 100);
                self.cursor -= 1;
                if self.cursor < 0 {
                    self.cursor = 2;
                }
            } else if mi.down {
                ctx.audio.play(sfx::MENU_MOVE, 100);
                self.cursor += 1;
                if self.cursor > 2 {
                    self.cursor = 0;
                }
            } else if select {
                ctx.audio.play(sfx::MENU_SELECT, 100);
                match self.cursor {
                    0 => self.process.start_fade_out(0),
                    1 => self.in_select = true,
                    _ => self.process.start_fade_out(1),
                }
            }
        } else {
            if mi.escape {
                select = true;
            }
            if mi.left {
                ctx.audio.play(sfx::MENU_MOVE, 100);
                ctx.app.current_level -= 1;
                if ctx.app.current_level < 0 {
                    ctx.app.current_level = ctx.app.max_sel_level;
                }
                self.map = PreviewMap::new(ctx);
            } else if mi.right {
                ctx.audio.play(sfx::MENU_MOVE, 100);
                ctx.app.current_level += 1;
                if ctx.app.max_sel_level < ctx.app.current_level {
                    ctx.app.current_level = 0;
                }
                self.map = PreviewMap::new(ctx);
            } else if select {
                ctx.audio.play(sfx::MENU_SELECT, 100);
                self.in_select = false;
            }
        }
    }

    fn render(&mut self, ctx: &mut Ctx) {
        if self.process.state == PState::Non {
            return;
        }
        ctx.gfx.set_3d(self.camera.view);
        self.background.render(ctx.gfx);
        self.map.map.mesh.render(ctx.gfx);
        ctx.gfx.set_2d();
        let (cx, cy) = (SCREEN_W / 2, SCREEN_H / 2);
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        self.text_level.set_pos(cx - 0x4c, cy - 200);
        self.text_level.draw(g, a);
        self.num_level.value = ctx.app.current_level + 1;
        self.num_level.x = cx - 0x44 + self.text_level.width;
        self.num_level.y = cy - 0xc6;
        self.num_level.draw(g, a);
        let step = self.level_led.width / 4;
        let mut x = cx - step * ctx.app.max_level / 2;
        let led_y = cy - 0xb4 + self.num_level.height();
        for i in 0..=ctx.app.max_level {
            self.level_led.frame = if i == ctx.app.current_level {
                2
            } else if ctx.app.max_sel_level < i {
                0
            } else {
                1
            };
            self.level_led.set_pos(x, led_y);
            self.level_led.draw(g, a);
            x += step;
        }
        let mut y = cy + 0x6e;
        for (i, m) in self.menu.iter_mut().enumerate() {
            m.set_center(cx, y);
            m.frame = (self.cursor == i as i32) as i32;
            m.draw(g, a);
            y += 8 + m.height;
        }
        if self.process.state == PState::Process {
            if !self.in_select {
                if self.cursor >= 0 {
                    let item = self.menu[self.cursor as usize].clone();
                    draw_cursor(ctx, &item, &mut self.cursor_left, &mut self.cursor_right);
                }
            } else {
                let tx = self.text_level.pos_x();
                let ty = self.text_level.pos_y();
                self.cursor2_right
                    .set_pos(tx - self.cursor2_right.width / 2 - 6, ty + 2);
                self.cursor2_right.draw(ctx.gfx, ctx.assets);
                self.cursor2_left
                    .set_pos(self.num_level.x + 6 + self.num_level.width(), ty + 2);
                self.cursor2_left.draw(ctx.gfx, ctx.assets);
            }
        }
        self.process.draw_fade(ctx.gfx, ctx.assets);
    }
}

// ---------------------------------------------------------------------------

pub struct Ending {
    process: Process,
    camera: Camera,
    background: TitleBackground,
    text_congra: Sprite,
    text_staff: Sprite,
    text_copyright: Sprite,
    chara: Vec<ObjX>,
    max_scale: [f32; 10],
    index: i32,
}

impl Ending {
    pub fn new(ctx: &mut Ctx) -> Ending {
        ctx.audio.play_music(0);
        let mut camera = Camera::default();
        camera.reset_for_title();
        let process = Process::new(ctx);
        let chara: Vec<ObjX> = [0x94, 0x9d, 0xa4, 0xaf, 0xd4, 0xad, 0xac, 0xa6, 0xb3, 0xb1]
            .iter()
            .map(|&id| {
                let mut o = ObjX::new(ctx.assets, Some(id));
                o.rotate_by(Axis::X, -HALF_PI);
                o
            })
            .collect();
        let m = 180.0f32;
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        let mut e = Ending {
            process,
            camera,
            background: TitleBackground::new(),
            text_congra: Sprite::new(g, a, 0x144, 1, true),
            text_staff: Sprite::new(g, a, 0x145, 1, true),
            text_copyright: Sprite::new(g, a, 0xf8, 1, true),
            chara,
            max_scale: [
                m,
                m,
                m * 1.5,
                m * 1.78,
                m * 1.1,
                m * 1.6,
                m * 1.4,
                m * 1.4,
                m * 0.65,
                m * 0.7,
            ],
            index: -1,
        };
        e.process.start_fade_in(0);
        e
    }
}

impl Scene for Ending {
    fn process_frame(&mut self, ctx: &mut Ctx, ticks: u32) {
        if self.process.state == PState::Non {
            return;
        }
        self.process.tick += ticks;
        match self.process.state {
            PState::FadeIn => {
                self.process.process_fade_in();
                if self.process.tick > 119 {
                    self.process.start_process(0);
                }
            }
            PState::FadeOut => {
                self.process.process_fade_out();
                if self.process.tick > 119 {
                    ctx.app.set_state(AppState::StartLogo);
                    self.process.state = PState::Non;
                    return;
                }
            }
            _ => {}
        }
        self.background.process_frame(ticks);
        if self.process.state != PState::Process {
            return;
        }
        let t = self.process.tick % 8000;
        self.index = (t / 800) as i32;
        let t = t % 800;
        let max = self.max_scale[self.index as usize];
        let mut s = if t < 100 {
            t as f32 / 100.0 * max
        } else if t < 700 {
            max
        } else {
            (800 - t) as f32 / 100.0 * max
        };
        if s < 1.0 {
            s = 1.0;
        }
        let c = &mut self.chara[self.index as usize];
        c.scale_to(s);
        c.rotate_by(Axis::Y, ticks as f32 * 0.013_962_6);
        let mi = menu_input(ctx);
        if mi.button || mi.escape {
            ctx.audio.play(sfx::MENU_SELECT, 100);
            self.process.start_fade_out(0);
        }
    }

    fn render(&mut self, ctx: &mut Ctx) {
        if self.process.state == PState::Non {
            return;
        }
        ctx.gfx.set_3d(self.camera.view);
        self.background.render(ctx.gfx);
        if self.index >= 0 {
            self.chara[self.index as usize].render(ctx.gfx);
        }
        ctx.gfx.set_2d();
        let (cx, cy) = (SCREEN_W / 2, SCREEN_H / 2);
        let (g, a) = (&mut *ctx.gfx, ctx.assets);
        self.text_congra.set_center(cx, cy - 0xa0);
        self.text_congra.draw(g, a);
        self.text_staff.set_center(cx, cy + 0x3c);
        self.text_staff.draw(g, a);
        self.text_copyright.set_center(cx, cy + 0xbe);
        self.text_copyright.draw(g, a);
        self.process.draw_fade(ctx.gfx, ctx.assets);
    }
}
