//! Atomic Sprocket (after Ben Farahmand's visualiser): a lit 3D spiral of boxes or spheres.
//!
//! Button1: spheres instead of boxes, Button2: noise colours instead of an HSB ramp,
//! Button3: slow (decaying) size response, Fader1: rotation responsiveness, Fader2: arc density,
//! Fader3: point size.
//!
//! Processing's P3D renderer did the 3D work in the original; here the boxes/spheres are
//! transformed, lit (ambient + two directional lights, as `lights()` + `directionalLight()`),
//! projected with Processing's default perspective camera, depth sorted and drawn as one mesh.

use super::{processing_draw, FrameCtx, LazyDrawLayer, VisBase, Visualiser};
use crate::gpu::{Blend, GpuFrame, RenderTarget};
use crate::util::{map, map_midi, pround, PColor, MIDI_MAX};
use nannou::color::LinSrgba;
use nannou::geom::{vec3, Vec3};
use nannou::glam::Mat4;

const SPHERE_DETAIL: usize = 8;

pub struct VisSprocket {
    base: VisBase,
    layer: LazyDrawLayer,
    angle: Vec<f32>,
    x: Vec<f32>,
    y: Vec<f32>,
    speed: f32,
    density: f32,
    scaling: f32,
    box_mesh: (Vec<Vec3>, Vec<[usize; 3]>, Vec<Vec3>),
    sphere_mesh: (Vec<Vec3>, Vec<[usize; 3]>),
}

/// Unit cube (edge 1) as 12 triangles with one normal per triangle.
fn unit_box() -> (Vec<Vec3>, Vec<[usize; 3]>, Vec<Vec3>) {
    let mut verts = Vec::new();
    let mut tris = Vec::new();
    let mut normals = Vec::new();
    let faces = [
        (Vec3::X, Vec3::Y, Vec3::Z),
        (-Vec3::X, Vec3::Y, -Vec3::Z),
        (Vec3::Y, Vec3::Z, Vec3::X),
        (-Vec3::Y, Vec3::Z, -Vec3::X),
        (Vec3::Z, Vec3::X, Vec3::Y),
        (-Vec3::Z, Vec3::X, -Vec3::Y),
    ];
    for (n, u, v) in faces {
        let base = verts.len();
        let c = n * 0.5;
        verts.push(c - u * 0.5 - v * 0.5);
        verts.push(c + u * 0.5 - v * 0.5);
        verts.push(c + u * 0.5 + v * 0.5);
        verts.push(c - u * 0.5 + v * 0.5);
        tris.push([base, base + 1, base + 2]);
        tris.push([base, base + 2, base + 3]);
        normals.push(n);
        normals.push(n);
    }
    (verts, tris, normals)
}

/// Unit sphere with `SPHERE_DETAIL` slices and stacks; the normal of each vertex is its position.
fn unit_sphere() -> (Vec<Vec3>, Vec<[usize; 3]>) {
    let (u, v) = (SPHERE_DETAIL, SPHERE_DETAIL);
    let mut verts = Vec::new();
    for j in 0..=v {
        let phi = std::f32::consts::PI * j as f32 / v as f32;
        for i in 0..=u {
            let theta = std::f32::consts::TAU * i as f32 / u as f32;
            verts.push(vec3(
                phi.sin() * theta.cos(),
                -phi.cos(),
                phi.sin() * theta.sin(),
            ));
        }
    }
    let mut tris = Vec::new();
    for j in 0..v {
        for i in 0..u {
            let a = j * (u + 1) + i;
            let b = a + u + 1;
            tris.push([a, b, a + 1]);
            tris.push([a + 1, b, b + 1]);
        }
    }
    (verts, tris)
}

/// Lighting from `lights()` (ambient 128, directional 128 along -z) plus
/// `directionalLight(225, 225, 225, 0, 0, -1)`.
fn light(normal: Vec3) -> f32 {
    let ndl = normal.normalize_or_zero().z.max(0.0);
    (128.0 + (128.0 + 225.0) * ndl) / 255.0
}

impl VisSprocket {
    pub fn new() -> Self {
        let spec = crate::audio::BUFFER_SIZE / 2 + 1;
        VisSprocket {
            base: VisBase::default(),
            layer: LazyDrawLayer::default(),
            angle: vec![0.0; spec],
            x: vec![0.0; spec],
            y: vec![0.0; spec],
            speed: 800.0,
            density: 1.0,
            scaling: 0.3,
            box_mesh: unit_box(),
            sphere_mesh: unit_sphere(),
        }
    }
}

impl Visualiser for VisSprocket {
    fn name(&self) -> &str {
        "Sprocket"
    }
    fn base(&mut self) -> &mut VisBase {
        &mut self.base
    }
    fn set_fader1(&mut self, v: i32) {
        self.speed = pround(map_midi(MIDI_MAX - v, 500.0, 10000.0)) as f32;
    }
    fn set_fader2(&mut self, v: i32) {
        self.density = pround(map_midi(MIDI_MAX - v, 1.0, 50.0)) as f32;
    }
    fn set_fader3(&mut self, v: i32) {
        self.scaling = map_midi(v, 0.2, 4.0);
    }

    fn render(&mut self, g: &mut GpuFrame, ctx: &FrameCtx) -> (&RenderTarget, Blend) {
        let (w, h) = (ctx.t.width, ctx.t.height);
        // Processing's default camera.
        let eye_z = (h / 2.0) / (std::f32::consts::PI / 6.0).tan();
        let project = |p: Vec3| -> Option<Vec3> {
            let dz = eye_z - p.z;
            if dz < 1.0 {
                return None;
            }
            let s = eye_z / dz;
            Some(vec3(
                w / 2.0 + (p.x - w / 2.0) * s,
                h / 2.0 + (p.y - h / 2.0) * s,
                p.z,
            ))
        };

        let spec = self.angle.len();
        let size = if self.base.button3 {
            ctx.fft.slow_levels[0] * self.scaling
        } else {
            ctx.fft.normalised_levels[0] * self.scaling
        };

        // (depth, [(screen point, colour); 3])
        let mut tris: Vec<(f32, [(Vec3, LinSrgba); 3])> = Vec::new();
        let mut m = Mat4::from_translation(vec3(w / 2.0, h / 2.0, 0.0));

        for i in 0..spec {
            let fill = if self.base.button2 {
                PColor::rgb(
                    ctx.noise.noise(i as f32 * 0.01, 0.0, 0.0) * 255.0,
                    ctx.noise.noise(i as f32 * 0.03, 0.0, 0.0) * 255.0,
                    ctx.noise.noise(i as f32 * 0.05, 0.0, 0.0) * 255.0,
                )
            } else {
                // colorMode(HSB) with the default 0..255 ranges
                PColor::hsb(
                    map(i as f32, 0.0, spec as f32, 10.0, 255.0) / 255.0,
                    150.0 / 255.0,
                    150.0 / 255.0,
                )
            };

            // `getFreq(i)` treats i as a frequency in Hz, so only the lowest bands are used.
            let volume = ctx.fft.fft.get_freq(i as f32);
            self.y[i] += volume / 20.0;
            self.x[i] += volume / 20.0;
            self.angle[i] += volume / self.speed;

            // The rotations accumulate across the loop, which is what makes the spiral.
            m = m
                * Mat4::from_rotation_x(self.angle[i].sin() / self.density)
                * Mat4::from_rotation_y(self.angle[i].cos() / self.density);

            if size <= 0.0 {
                continue;
            }
            let local =
                m * Mat4::from_translation(vec3((self.x[i] % w) / 3.5, (self.y[i] % h) / 3.5, 0.0));

            let shade = |l: f32| {
                LinSrgba::new(
                    (fill.r * l).min(1.0),
                    (fill.g * l).min(1.0),
                    (fill.b * l).min(1.0),
                    1.0,
                )
            };

            if !self.base.button1 {
                let model = local * Mat4::from_scale(Vec3::splat(size));
                let (verts, idx, normals) = &self.box_mesh;
                for (t, n) in idx.iter().zip(normals) {
                    let col = shade(light(local.transform_vector3(*n)));
                    let pts: Option<Vec<Vec3>> = t
                        .iter()
                        .map(|k| project(model.transform_point3(verts[*k])))
                        .collect();
                    if let Some(p) = pts {
                        let depth = (p[0].z + p[1].z + p[2].z) / 3.0;
                        tris.push((depth, [(p[0], col), (p[1], col), (p[2], col)]));
                    }
                }
            } else {
                // sphere(size): Processing's sphere takes a radius
                let model = local * Mat4::from_scale(Vec3::splat(size));
                let (verts, idx) = &self.sphere_mesh;
                for t in idx {
                    let mut out = [(Vec3::ZERO, LinSrgba::new(0.0, 0.0, 0.0, 1.0)); 3];
                    let mut ok = true;
                    for (o, k) in out.iter_mut().zip(t) {
                        match project(model.transform_point3(verts[*k])) {
                            Some(p) => *o = (p, shade(light(local.transform_vector3(verts[*k])))),
                            None => ok = false,
                        }
                    }
                    if ok {
                        let depth = (out[0].0.z + out[1].0.z + out[2].0.z) / 3.0;
                        tris.push((depth, out));
                    }
                }
            }
        }

        // Painter's algorithm: far triangles first.
        tris.sort_by(|a, b| a.0.total_cmp(&b.0));

        let draw = processing_draw(w, h);
        draw.background().color(LinSrgba::new(0.0, 0.0, 0.0, 0.0));
        if !tris.is_empty() {
            let mut points = Vec::with_capacity(tris.len() * 3);
            for (_, t) in &tris {
                for (p, c) in t {
                    points.push((vec3(p.x, p.y, 0.0), *c));
                }
            }
            let indices: Vec<usize> = (0..points.len()).collect();
            draw.mesh().indexed_colored(points, indices);
        }
        (
            self.layer.render(g, &draw, "Sprocket"),
            Blend::Premultiplied,
        )
    }
}
