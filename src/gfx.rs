//! Rendering backend: reproduces the Direct3D 7 fixed-function states the
//! game used (pre-lit textured quads, lit material meshes, additive
//! particles, multiplicative shadows) on top of macroquad/miniquad, plus the
//! DirectDraw colour-keyed sprite blits for the 2D overlay.

use std::collections::HashMap;

use macroquad::miniquad::{
    Backend, BlendFactor, BlendState, BlendValue, BufferId, BufferLayout, BufferSource, BufferType,
    BufferUsage, Comparison, Equation, Pipeline, PipelineParams, RenderingBackend, ShaderMeta,
    ShaderSource, TextureFormat, TextureId, TextureKind, TextureParams, TextureWrap,
    UniformBlockLayout, UniformDesc, UniformType, UniformsSource, VertexAttribute, VertexFormat,
};
use macroquad::prelude::{
    Camera, Color, DrawTextureParams, FilterMode, Mat4, Rect, Texture2D, draw_texture_ex,
    gl_use_default_material, screen_height, screen_width, set_camera, vec2,
};
use macroquad::texture::RenderPass;

use crate::assets::Assets;
use crate::assets::bmp::Image;
use crate::math::{Mat, Vec2, Vec3};

pub const SCREEN_W: i32 = 640;
pub const SCREEN_H: i32 = 480;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    /// Opaque, depth write.
    Opaque,
    /// Alpha blended translucent materials, depth write (CD3DFile alpha pass).
    AlphaWrite,
    /// `ALPHA_PARTICLE`: ONE/ONE, depth test without depth write.
    Additive,
    /// `ALPHA_SHADOW`: ZERO/SRCCOLOR, depth test without depth write.
    Shadow,
    /// `ALPHA_TRANSPARENT`: SRCALPHA/INVSRCALPHA, depth test without write.
    Transparent,
}

impl Blend {
    const ALL: [Blend; 5] = [
        Blend::Opaque,
        Blend::AlphaWrite,
        Blend::Additive,
        Blend::Shadow,
        Blend::Transparent,
    ];

    fn writes_depth(self) -> bool {
        matches!(self, Blend::Opaque | Blend::AlphaWrite)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct LVertex {
    pub pos: Vec3,
    pub color: [u8; 4],
    pub uv: Vec2,
}

/// GPU vertex layout (`D3DLVERTEX` without the specular term).
#[repr(C)]
#[derive(Clone, Copy)]
struct GpuVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [u8; 4],
}

#[repr(C)]
struct Uniforms {
    mvp: Mat4,
}

const VERTEX: &str = r#"#version 100
attribute vec3 in_pos;
attribute vec2 in_uv;
attribute vec4 in_color;
varying lowp vec2 uv;
varying lowp vec4 color;
uniform mat4 mvp;
void main() {
    gl_Position = mvp * vec4(in_pos, 1.0);
    color = in_color / 255.0;
    uv = in_uv;
}"#;

const FRAGMENT: &str = r#"#version 100
varying lowp vec4 color;
varying lowp vec2 uv;
uniform sampler2D tex;
void main() {
    gl_FragColor = color * texture2D(tex, uv);
}"#;

pub struct Sprite2DSurface {
    pub tex: Texture2D,
    pub width: i32,
    pub height: i32,
}

/// Immediate-mode 3D renderer on raw miniquad. macroquad's batcher cannot
/// express "depth test without depth write" (miniquad disables the depth
/// test whenever depth writes are off), which D3D used for shadows and
/// particles, so 3D geometry bypasses it and toggles `glDepthMask` itself.
struct Renderer3D {
    pipelines: HashMap<Blend, Pipeline>,
    vbuf: BufferId,
    ibuf: BufferId,
    vcap: usize,
    icap: usize,
    gl: bool,
    mvp: Mat4,
}

pub struct Gfx {
    r3d: Renderer3D,
    textures: HashMap<(u16, bool), TextureId>,
    images: HashMap<u16, Image>,
    surfaces: HashMap<(u16, bool), Sprite2DSurface>,
    white: TextureId,
    pub view: Mat,
    pub proj: Mat,
    viewport: (i32, i32, i32, i32),
}

fn pipeline_params(blend: Blend, gl: bool) -> PipelineParams {
    let alpha = BlendState::new(
        Equation::Add,
        BlendFactor::Value(BlendValue::SourceAlpha),
        BlendFactor::OneMinusValue(BlendValue::SourceAlpha),
    );
    let color_blend = match blend {
        Blend::Opaque => None,
        Blend::AlphaWrite | Blend::Transparent => Some(alpha),
        Blend::Additive => Some(BlendState::new(
            Equation::Add,
            BlendFactor::One,
            BlendFactor::One,
        )),
        Blend::Shadow => Some(BlendState::new(
            Equation::Add,
            BlendFactor::Zero,
            BlendFactor::Value(BlendValue::SourceColor),
        )),
    };
    PipelineParams {
        depth_test: Comparison::LessOrEqual,
        // On GL the depth mask is toggled manually; elsewhere fall back to
        // miniquad's semantics (no test for non-writing passes).
        depth_write: gl || blend.writes_depth(),
        color_blend,
        alpha_blend: color_blend,
        ..Default::default()
    }
}

fn ctx() -> &'static mut dyn RenderingBackend {
    // SAFETY: macroquad is single threaded; the context lives for the
    // whole program and is only touched from the main thread.
    unsafe { macroquad::window::get_internal_gl().quad_context }
}

impl Renderer3D {
    fn new() -> Renderer3D {
        let ctx = ctx();
        let gl = ctx.info().backend == Backend::OpenGl;
        let shader = ctx
            .new_shader(
                ShaderSource::Glsl {
                    vertex: VERTEX,
                    fragment: FRAGMENT,
                },
                ShaderMeta {
                    images: vec!["tex".to_string()],
                    uniforms: UniformBlockLayout {
                        uniforms: vec![UniformDesc::new("mvp", UniformType::Mat4)],
                    },
                },
            )
            .expect("3D shader");
        let attrs = [
            VertexAttribute::new("in_pos", VertexFormat::Float3),
            VertexAttribute::new("in_uv", VertexFormat::Float2),
            VertexAttribute::new("in_color", VertexFormat::Byte4),
        ];
        let mut pipelines = HashMap::new();
        for b in Blend::ALL {
            let p = ctx.new_pipeline(
                &[BufferLayout::default()],
                &attrs,
                shader,
                pipeline_params(b, gl),
            );
            pipelines.insert(b, p);
        }
        let (vcap, icap) = (4096, 8192);
        let vbuf = ctx.new_buffer(
            BufferType::VertexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<GpuVertex>(vcap),
        );
        let ibuf = ctx.new_buffer(
            BufferType::IndexBuffer,
            BufferUsage::Stream,
            BufferSource::empty::<u16>(icap),
        );
        Renderer3D {
            pipelines,
            vbuf,
            ibuf,
            vcap,
            icap,
            gl,
            mvp: Mat4::IDENTITY,
        }
    }

    fn draw(
        &mut self,
        viewport: (i32, i32, i32, i32),
        blend: Blend,
        tex: TextureId,
        verts: &[GpuVertex],
        idx: &[u16],
    ) {
        // Anything macroquad queued so far must reach the GPU first.
        unsafe { macroquad::window::get_internal_gl().flush() };
        let ctx = ctx();
        if verts.len() > self.vcap {
            ctx.delete_buffer(self.vbuf);
            self.vcap = verts.len().next_power_of_two();
            self.vbuf = ctx.new_buffer(
                BufferType::VertexBuffer,
                BufferUsage::Stream,
                BufferSource::empty::<GpuVertex>(self.vcap),
            );
        }
        if idx.len() > self.icap {
            ctx.delete_buffer(self.ibuf);
            self.icap = idx.len().next_power_of_two();
            self.ibuf = ctx.new_buffer(
                BufferType::IndexBuffer,
                BufferUsage::Stream,
                BufferSource::empty::<u16>(self.icap),
            );
        }
        ctx.buffer_update(self.vbuf, BufferSource::slice(verts));
        ctx.buffer_update(self.ibuf, BufferSource::slice(idx));
        ctx.begin_default_pass(macroquad::miniquad::PassAction::Nothing);
        let (x, y, w, h) = viewport;
        ctx.apply_viewport(x, y, w, h);
        ctx.apply_pipeline(&self.pipelines[&blend]);
        ctx.apply_scissor_rect(x, y, w, h);
        ctx.apply_bindings(&macroquad::miniquad::Bindings {
            vertex_buffers: vec![self.vbuf],
            index_buffer: self.ibuf,
            images: vec![tex],
        });
        ctx.apply_uniforms(UniformsSource::table(&Uniforms { mvp: self.mvp }));
        let no_write = self.gl && !blend.writes_depth();
        if no_write {
            unsafe { macroquad::miniquad::native::gl::glDepthMask(0) };
        }
        ctx.draw(0, idx.len() as i32, 1);
        if no_write {
            unsafe { macroquad::miniquad::native::gl::glDepthMask(1) };
        }
        ctx.end_render_pass();
    }
}

/// D3DTextr flags: `D3DTEXTR_TRANSPARENTBLACK` makes black texels transparent.
pub const TEXTR_TRANSPARENT_BLACK: u32 = 2;

impl Gfx {
    pub fn new() -> Gfx {
        let r3d = Renderer3D::new();
        let white = ctx().new_texture_from_rgba8(1, 1, &[255, 255, 255, 255]);
        Gfx {
            r3d,
            textures: HashMap::new(),
            images: HashMap::new(),
            surfaces: HashMap::new(),
            white,
            view: Mat::IDENTITY,
            proj: Mat::IDENTITY,
            viewport: (0, 0, SCREEN_W, SCREEN_H),
        }
    }

    /// `D3DTextr_CreateTextureFromResource`.
    pub fn load_texture(&mut self, assets: &Assets, id: u16, flags: u32) {
        if self.images.contains_key(&id) {
            return;
        }
        if let Some(mut img) = assets.bitmap(id) {
            if flags & TEXTR_TRANSPARENT_BLACK != 0 {
                for px in img.rgba.chunks_exact_mut(4) {
                    if px[0] == 0 && px[1] == 0 && px[2] == 0 {
                        px[3] = 0;
                    }
                }
            }
            self.images.insert(id, img);
        }
    }

    fn texture(&mut self, id: u16, linear: bool) -> TextureId {
        if id == 0 {
            return self.white;
        }
        if let Some(t) = self.textures.get(&(id, linear)) {
            return *t;
        }
        let Some(img) = self.images.get(&id) else {
            return self.white;
        };
        let filter = if linear {
            macroquad::miniquad::FilterMode::Linear
        } else {
            macroquad::miniquad::FilterMode::Nearest
        };
        let t = ctx().new_texture_from_data_and_format(
            &img.rgba,
            TextureParams {
                kind: TextureKind::Texture2D,
                format: TextureFormat::RGBA8,
                wrap: TextureWrap::Repeat,
                min_filter: filter,
                mag_filter: filter,
                width: img.width,
                height: img.height,
                ..Default::default()
            },
        );
        self.textures.insert((id, linear), t);
        t
    }

    /// DirectDraw surface for a bitmap resource (`JSurface`). With a colour
    /// key the top-left pixel colour becomes transparent (CLR_INVALID key).
    pub fn surface(&mut self, assets: &Assets, id: u16, color_key: bool) -> &Sprite2DSurface {
        self.surfaces.entry((id, color_key)).or_insert_with(|| {
            let mut img = assets.bitmap(id).unwrap_or(Image {
                width: 1,
                height: 1,
                rgba: vec![0; 4],
            });
            if color_key {
                let k = img.pixel(0, 0);
                for px in img.rgba.chunks_exact_mut(4) {
                    if px[0] == k[0] && px[1] == k[1] && px[2] == k[2] {
                        px[3] = 0;
                    }
                }
            }
            let tex = Texture2D::from_rgba8(img.width as u16, img.height as u16, &img.rgba);
            tex.set_filter(FilterMode::Nearest);
            Sprite2DSurface {
                tex,
                width: img.width as i32,
                height: img.height as i32,
            }
        })
    }

    pub fn surface_size(&mut self, assets: &Assets, id: u16, color_key: bool) -> (i32, i32) {
        let s = self.surface(assets, id, color_key);
        (s.width, s.height)
    }

    fn compute_viewport() -> (i32, i32, i32, i32) {
        let (w, h) = (screen_width(), screen_height());
        let scale = (w / SCREEN_W as f32).min(h / SCREEN_H as f32).max(0.01);
        let vw = (SCREEN_W as f32 * scale).round() as i32;
        let vh = (SCREEN_H as f32 * scale).round() as i32;
        (((w as i32) - vw) / 2, ((h as i32) - vh) / 2, vw, vh)
    }

    pub fn begin_frame(&mut self) {
        self.viewport = Gfx::compute_viewport();
        macroquad::prelude::set_default_camera();
        // Original clear colour 0x2d1d62 (also clears depth).
        macroquad::prelude::clear_background(Color::from_rgba(0x2d, 0x1d, 0x62, 255));
    }

    /// Paints the letterbox bars outside the 640x480 game area.
    pub fn end_frame(&mut self) {
        gl_use_default_material();
        macroquad::prelude::set_default_camera();
        let (x, y, w, h) = self.viewport;
        let (sw, sh) = (screen_width(), screen_height());
        let black = Color::from_rgba(0, 0, 0, 255);
        // The viewport is given in GL (bottom-left) coordinates; the bars are
        // symmetric so only their sizes matter.
        let (x, y, w, h) = (x as f32, y as f32, w as f32, h as f32);
        macroquad::prelude::draw_rectangle(0.0, 0.0, x, sh, black);
        macroquad::prelude::draw_rectangle(x + w, 0.0, sw - x - w, sh, black);
        macroquad::prelude::draw_rectangle(0.0, 0.0, sw, y, black);
        macroquad::prelude::draw_rectangle(0.0, y + h, sw, sh - y - h, black);
    }

    /// Selects the 3D camera; `view` comes from `XCamera`.
    pub fn set_3d(&mut self, view: Mat) {
        self.view = view;
        // D3DUtil_SetProjectionMatrix(42 degrees, 3:4, 10, 2000).
        self.proj = Mat::projection(0.733_038_3, 0.75, 10.0, 2000.0);
        self.r3d.mvp = view.mul(&self.proj).to_gl();
    }

    pub fn set_2d(&mut self) {
        gl_use_default_material();
        let m = Mat4::orthographic_rh_gl(0.0, SCREEN_W as f32, SCREEN_H as f32, 0.0, -1.0, 1.0);
        struct Cam2D(Mat4, (i32, i32, i32, i32));
        impl Camera for Cam2D {
            fn matrix(&self) -> Mat4 {
                self.0
            }
            fn depth_enabled(&self) -> bool {
                false
            }
            fn render_pass(&self) -> Option<RenderPass> {
                None
            }
            fn viewport(&self) -> Option<(i32, i32, i32, i32)> {
                Some(self.1)
            }
        }
        set_camera(&Cam2D(m, self.viewport));
    }

    /// Draws world-space triangles with the given texture and blend state.
    pub fn draw_tris(
        &mut self,
        blend: Blend,
        tex: u16,
        linear: bool,
        verts: &[LVertex],
        idx: &[u16],
    ) {
        if verts.is_empty() || idx.is_empty() {
            return;
        }
        let texture = self.texture(tex, linear);
        let gpu: Vec<GpuVertex> = verts
            .iter()
            .map(|v| GpuVertex {
                pos: v.pos.to_array(),
                uv: v.uv.to_array(),
                color: v.color,
            })
            .collect();
        self.r3d.draw(self.viewport, blend, texture, &gpu, idx);
    }

    /// `IDirectDrawSurface7::Blt` of one sprite frame (source rect) into the
    /// destination rect, honouring the colour key.
    pub fn blit(
        &mut self,
        assets: &Assets,
        id: u16,
        color_key: bool,
        src: (i32, i32, i32, i32),
        dst: (i32, i32, i32, i32),
    ) {
        let tex = self.surface(assets, id, color_key).tex.clone();
        draw_texture_ex(
            &tex,
            dst.0 as f32,
            dst.1 as f32,
            Color::from_rgba(255, 255, 255, 255),
            DrawTextureParams {
                dest_size: Some(vec2(dst.2 as f32, dst.3 as f32)),
                source: Some(Rect::new(
                    src.0 as f32,
                    src.1 as f32,
                    src.2 as f32,
                    src.3 as f32,
                )),
                ..Default::default()
            },
        );
    }
}
