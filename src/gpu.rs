//! GPU plumbing: offscreen layers (the equivalent of Processing's `PGraphics`), a blitter for
//! compositing them, and `ShaderPass`, which runs a translated GLSL fragment shader full-screen
//! (Processing's `filter(shader)`).
//!
//! Colour handling: every offscreen layer is plain `Rgba8Unorm` holding raw 0..1 channel values,
//! exactly like Processing's framebuffers, and blending happens on those raw values. Draw-rendered
//! layers hold premultiplied colour; shader-rendered layers hold straight alpha. Only the final
//! present pass converts to the window's (linear) frame format.

use crate::shader::glsl::{self, UniformKind, UniformLayout, FULLSCREEN_VS_WGSL};
use nannou::draw;
use nannou::wgpu;
use std::borrow::Cow;
use std::collections::HashMap;
use wgpu_upstream as wgpu_up;

pub const LAYER_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
pub const DRAW_MSAA_SAMPLES: u32 = 4;

/// How a pass combines its output with what is already in the target.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Blend {
    /// Overwrite (Processing's `filter()`).
    Replace,
    /// Source holds straight alpha (shader output, images).
    Alpha,
    /// Source holds premultiplied alpha (nannou Draw output).
    Premultiplied,
}

impl Blend {
    fn state(self) -> Option<wgpu::BlendState> {
        let over = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        };
        match self {
            Blend::Replace => None,
            Blend::Alpha => Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: over,
            }),
            Blend::Premultiplied => Some(wgpu::BlendState {
                color: over,
                alpha: over,
            }),
        }
    }
}

/// A single-sampled texture that can be rendered to and sampled from.
pub struct RenderTarget {
    pub texture: wgpu::Texture,
    pub view: wgpu::TextureView,
    pub size: [u32; 2],
}

impl RenderTarget {
    pub fn new(device: &wgpu::Device, size: [u32; 2], label: &str) -> Self {
        Self::with_samples(device, size, 1, label)
    }

    fn with_samples(device: &wgpu::Device, size: [u32; 2], samples: u32, label: &str) -> Self {
        let mut usage = wgpu::TextureUsages::RENDER_ATTACHMENT;
        if samples == 1 {
            usage |= wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC;
        }
        let _ = label;
        let texture = wgpu::TextureBuilder::new()
            .size(size)
            .dimension(wgpu::TextureDimension::D2)
            .sample_count(samples)
            .format(LAYER_FORMAT)
            .usage(usage)
            .build(device);
        let view = texture.view().build();
        RenderTarget {
            texture,
            view,
            size,
        }
    }

    /// Upload an RGBA8 image (e.g. a hotcue PNG) as a sampleable texture.
    pub fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        size: [u32; 2],
        rgba: &[u8],
        label: &str,
    ) -> Self {
        let t = Self::new(device, size, label);
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &t.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * size[0]),
                rows_per_image: Some(size[1]),
            },
            wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
        );
        t
    }

    /// Clear to a colour (raw 0..1 values).
    pub fn clear(&self, encoder: &mut wgpu::CommandEncoder, rgba: [f64; 4]) {
        clear_view(encoder, &self.view, rgba);
    }
}

pub fn clear_view(encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, rgba: [f64; 4]) {
    let _ = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("clear"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color {
                    r: rgba[0],
                    g: rgba[1],
                    b: rgba[2],
                    a: rgba[3],
                }),
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    });
}

/// A layer rendered with nannou's `Draw` API, multisampled for smooth edges and resolved into a
/// sampleable texture. The MSAA texture persists between frames, so a visualiser that doesn't
/// clear (Oblivion's trails) keeps its history.
pub struct DrawLayer {
    msaa: RenderTarget,
    pub resolved: RenderTarget,
}

impl DrawLayer {
    pub fn new(device: &wgpu::Device, size: [u32; 2], label: &str) -> Self {
        DrawLayer {
            msaa: RenderTarget::with_samples(device, size, DRAW_MSAA_SAMPLES, label),
            resolved: RenderTarget::new(device, size, label),
        }
    }

    pub fn render(&self, g: &mut GpuFrame, draw: &draw::Draw) {
        g.draw_renderer.encode_render_pass(
            g.device,
            g.encoder,
            draw,
            1.0,
            self.msaa.size,
            &self.msaa.view,
            Some(&self.resolved.view),
        );
    }
}

/// Per-frame access to the GPU, handed to everything that renders.
pub struct GpuFrame<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub draw_renderer: &'a mut draw::Renderer,
    pub blitter: &'a mut Blitter,
    pub size: [u32; 2],
}

impl GpuFrame<'_> {
    /// Composite `src` onto `dst`, multiplying by `tint` (raw rgba).
    pub fn blit(
        &mut self,
        src: &wgpu::TextureView,
        dst: &wgpu::TextureView,
        tint: [f32; 4],
        blend: Blend,
    ) {
        self.blitter.blit(
            self.device,
            self.queue,
            self.encoder,
            src,
            dst,
            LAYER_FORMAT,
            tint,
            blend,
            false,
        );
    }
}

fn sampler(device: &wgpu::Device) -> wgpu::Sampler {
    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("gsr linear clamp"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    })
}

fn bind_group_layout(device: &wgpu::Device, label: &str) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some(label),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 2,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu_up::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}

#[allow(clippy::too_many_arguments)]
fn create_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    vs: &wgpu::ShaderModule,
    fs: &wgpu::ShaderModule,
    fs_entry: &str,
    format: wgpu::TextureFormat,
    blend: Blend,
    label: &str,
) -> wgpu::RenderPipeline {
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(label),
        bind_group_layouts: &[layout],
        push_constant_ranges: &[],
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(label),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: vs,
            entry_point: "main",
            buffers: &[],
        },
        fragment: Some(wgpu::FragmentState {
            module: fs,
            entry_point: fs_entry,
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: blend.state(),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    })
}

fn run_pass(
    encoder: &mut wgpu::CommandEncoder,
    pipeline: &wgpu::RenderPipeline,
    bind_group: &wgpu::BindGroup,
    dst: &wgpu::TextureView,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("gsr fullscreen pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: dst,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: true,
            },
        })],
        depth_stencil_attachment: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

const BLIT_WGSL: &str = r#"
struct Params {
    tint: vec4<f32>,
    flags: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var tex: texture_2d<f32>;
@group(0) @binding(2) var samp: sampler;

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((c + vec3<f32>(0.055)) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

@fragment
fn main(@location(0) uv: vec4<f32>) -> @location(0) vec4<f32> {
    var c = textureSample(tex, samp, vec2<f32>(uv.x, 1.0 - uv.y)) * params.tint;
    if (params.flags.x > 0.5) {
        c = vec4<f32>(srgb_to_linear(clamp(c.rgb, vec3<f32>(0.0), vec3<f32>(1.0))), 1.0);
    }
    return c;
}
"#;

/// Draws one texture over another. Keeps a small ring of uniform buffers because a blit can run
/// many times per frame with different parameters (all writes land before the encoder runs).
pub struct Blitter {
    vs: wgpu::ShaderModule,
    fs: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipelines: HashMap<(wgpu::TextureFormat, Blend), wgpu::RenderPipeline>,
    buffers: Vec<wgpu::Buffer>,
    next: usize,
}

impl Blitter {
    pub fn new(device: &wgpu::Device) -> Self {
        let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gsr fullscreen vs"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(FULLSCREEN_VS_WGSL)),
        });
        let fs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gsr blit fs"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(BLIT_WGSL)),
        });
        Blitter {
            vs,
            fs,
            layout: bind_group_layout(device, "gsr blit"),
            sampler: sampler(device),
            pipelines: HashMap::new(),
            buffers: Vec::new(),
            next: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.next = 0;
    }

    #[allow(clippy::too_many_arguments)]
    pub fn blit(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        src: &wgpu::TextureView,
        dst: &wgpu::TextureView,
        dst_format: wgpu::TextureFormat,
        tint: [f32; 4],
        blend: Blend,
        decode_srgb: bool,
    ) {
        if self.next == self.buffers.len() {
            self.buffers
                .push(device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("gsr blit params"),
                    size: 32,
                    usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }));
        }
        let buffer = &self.buffers[self.next];
        self.next += 1;
        let mut data = [0f32; 8];
        data[..4].copy_from_slice(&tint);
        data[4] = if decode_srgb { 1.0 } else { 0.0 };
        queue.write_buffer(buffer, 0, as_bytes(&data));

        let (vs, fs, layout) = (&self.vs, &self.fs, &self.layout);
        let pipeline = self
            .pipelines
            .entry((dst_format, blend))
            .or_insert_with(|| {
                create_pipeline(
                    device, layout, vs, fs, "main", dst_format, blend, "gsr blit",
                )
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gsr blit"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        run_pass(encoder, pipeline, &bind_group, dst);
    }
}

fn as_bytes(data: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding or invalid bit patterns.
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}

/// A full-screen fragment shader loaded from a Processing GLSL file, the equivalent of `PShader`
/// used with `filter()`. Uniform values persist between frames like `PShader.set()`.
pub struct ShaderPass {
    pub name: String,
    layout: UniformLayout,
    data: Vec<u8>,
    buffer: wgpu::Buffer,
    bind_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    vs: wgpu::ShaderModule,
    fs: wgpu::ShaderModule,
    pipelines: HashMap<Blend, wgpu::RenderPipeline>,
    /// 1x1 transparent texture bound when the shader is run without a source.
    dummy: RenderTarget,
}

impl ShaderPass {
    /// Load and translate `data/Shaders/<file>`. Errors carry the file name and the parser output.
    pub fn load(
        device: &wgpu::Device,
        shader_dir: &std::path::Path,
        file: &str,
    ) -> anyhow::Result<Self> {
        let path = shader_dir.join(file);
        let src = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        let t = glsl::translate(&src).map_err(|e| anyhow::anyhow!("{file}: {e}"))?;
        let vs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gsr fullscreen vs"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(FULLSCREEN_VS_WGSL)),
        });
        let fs = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(file),
            source: wgpu::ShaderSource::Wgsl(Cow::Owned(t.wgsl)),
        });
        let size = t.layout.size.max(16) as usize;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(file),
            size: size as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Ok(ShaderPass {
            name: file.to_string(),
            layout: t.layout,
            data: vec![0; size],
            buffer,
            bind_layout: bind_group_layout(device, file),
            sampler: sampler(device),
            vs,
            fs,
            pipelines: HashMap::new(),
            dummy: RenderTarget::new(device, [1, 1], "gsr dummy"),
        })
    }

    fn write(&mut self, name: &str, expected: &[UniformKind], words: &[[u8; 4]]) {
        let Some(field) = self.layout.fields.get(name) else {
            return; // like PShader.set(): unknown uniforms are ignored
        };
        if !expected.contains(&field.kind) {
            log::debug!("{}: uniform '{name}' is {:?}", self.name, field.kind);
        }
        let mut off = field.offset as usize;
        for w in words {
            if off + 4 <= self.data.len() {
                self.data[off..off + 4].copy_from_slice(w);
            }
            off += 4;
        }
    }

    pub fn set_f32(&mut self, name: &str, v: f32) {
        // ints and bools declared in GLSL are stored as i32
        match self.layout.fields.get(name).map(|f| f.kind) {
            Some(UniformKind::Int) | Some(UniformKind::Bool) => self.set_i32(name, v as i32),
            _ => self.write(name, &[UniformKind::Float], &[v.to_ne_bytes()]),
        }
    }

    pub fn set_i32(&mut self, name: &str, v: i32) {
        match self.layout.fields.get(name).map(|f| f.kind) {
            Some(UniformKind::Float) => {
                self.write(name, &[UniformKind::Float], &[(v as f32).to_ne_bytes()])
            }
            _ => self.write(
                name,
                &[UniformKind::Int, UniformKind::Bool],
                &[v.to_ne_bytes()],
            ),
        }
    }

    pub fn set_bool(&mut self, name: &str, v: bool) {
        self.set_i32(name, v as i32);
    }

    pub fn set_vec2(&mut self, name: &str, x: f32, y: f32) {
        self.write(
            name,
            &[UniformKind::Vec2],
            &[x.to_ne_bytes(), y.to_ne_bytes()],
        );
    }

    pub fn set_vec3(&mut self, name: &str, x: f32, y: f32, z: f32) {
        self.write(
            name,
            &[UniformKind::Vec3],
            &[x.to_ne_bytes(), y.to_ne_bytes(), z.to_ne_bytes()],
        );
    }

    pub fn set_vec4(&mut self, name: &str, x: f32, y: f32, z: f32, w: f32) {
        self.write(
            name,
            &[UniformKind::Vec4],
            &[
                x.to_ne_bytes(),
                y.to_ne_bytes(),
                z.to_ne_bytes(),
                w.to_ne_bytes(),
            ],
        );
    }

    /// Run the shader over the whole of `dst`, sampling `src` as `texture`.
    pub fn encode(
        &mut self,
        g: &mut GpuFrame,
        src: Option<&RenderTarget>,
        dst: &wgpu::TextureView,
        dst_size: [u32; 2],
        blend: Blend,
    ) {
        let src_size = src.map(|s| s.size).unwrap_or(dst_size);
        self.set_vec2(
            "texOffset",
            1.0 / src_size[0] as f32,
            1.0 / src_size[1] as f32,
        );
        self.set_vec2(
            glsl::TARGET_SIZE_UNIFORM,
            dst_size[0] as f32,
            dst_size[1] as f32,
        );
        g.queue.write_buffer(&self.buffer, 0, &self.data);

        let device = g.device;
        let (vs, fs, layout, name) = (&self.vs, &self.fs, &self.bind_layout, &self.name);
        let pipeline = self.pipelines.entry(blend).or_insert_with(|| {
            create_pipeline(device, layout, vs, fs, "main", LAYER_FORMAT, blend, name)
        });
        let src_view = src.map(|s| &s.view).unwrap_or(&self.dummy.view);
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(&self.name),
            layout: &self.bind_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(src_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        run_pass(g.encoder, pipeline, &bind_group, dst);
    }
}

/// Two render targets used alternately so a shader can read last frame's output (Processing's
/// `pg.filter()` reads the PGraphics it writes to).
pub struct PingPong {
    targets: [RenderTarget; 2],
    current: usize,
}

impl PingPong {
    pub fn new(device: &wgpu::Device, size: [u32; 2], label: &str) -> Self {
        PingPong {
            targets: [
                RenderTarget::new(device, size, label),
                RenderTarget::new(device, size, label),
            ],
            current: 0,
        }
    }

    pub fn current(&self) -> &RenderTarget {
        &self.targets[self.current]
    }

    /// Run `pass` from the current target into the other one and make that current.
    pub fn apply(&mut self, g: &mut GpuFrame, pass: &mut ShaderPass) {
        let (a, b) = self.targets.split_at_mut(1);
        let (src, dst) = if self.current == 0 {
            (&a[0], &b[0])
        } else {
            (&b[0], &a[0])
        };
        pass.encode(g, Some(src), &dst.view, dst.size, Blend::Replace);
        self.current = 1 - self.current;
    }
}
