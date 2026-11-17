//! Dancing stick figures (StickFigures.pde): a crowd of up to 121 figures laid out in one of four
//! formations, each stepping through dance poses from `dancemoves.json` on the beat.

use super::FrameCtx;
use crate::config::{as_i64, load_json};
use crate::util::{lerp, map_midi, random, PColor, MIDI_MAX};
use nannou::draw::Draw;
use nannou::geom::{vec2, vec3, Vec2};
use std::f32::consts::TAU;
use std::path::Path;

const NUM_PARTS: usize = 11;
pub const MAX_CROWD_SIZE: usize = 121;

type Pose = [Vec2; NUM_PARTS];

// ---------------------------------------------------------------------------------------------
// Dance routines

pub struct Routine {
    moves: Vec<Pose>,
    index: usize,
}

impl Routine {
    fn get(&self) -> &Pose {
        &self.moves[self.index]
    }
    /// Step to the next pose; true when the routine wrapped around.
    fn on_beat(&mut self) -> bool {
        self.index += 1;
        if self.index >= self.moves.len() {
            self.index = 0;
            true
        } else {
            false
        }
    }
}

pub struct DanceRoutines {
    routines: Vec<Routine>,
    index: usize,
}

impl DanceRoutines {
    pub fn load(data: &Path) -> anyhow::Result<Self> {
        let json = load_json(&data.join("dancemoves.json"))?;
        let mut routines = Vec::new();
        for routine in json.as_array().into_iter().flatten() {
            let mut moves = Vec::new();
            for pose in routine.as_array().into_iter().flatten() {
                let mut p = [Vec2::ZERO; NUM_PARTS];
                for (j, part) in pose
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(NUM_PARTS)
                    .enumerate()
                {
                    let x = part.get("pos_x").and_then(as_i64).unwrap_or(0) as f32;
                    let y = part.get("pos_y").and_then(as_i64).unwrap_or(0) as f32;
                    p[j] = vec2(x, y);
                }
                moves.push(p);
            }
            if !moves.is_empty() {
                routines.push(Routine { moves, index: 0 });
            }
        }
        if routines.is_empty() {
            anyhow::bail!("no dance routines in dancemoves.json");
        }
        Ok(DanceRoutines { routines, index: 0 })
    }

    /// Fallback when dancemoves.json is missing: stand still.
    pub fn standing() -> Self {
        DanceRoutines {
            routines: vec![Routine {
                moves: vec![StickFigure::origin_pose()],
                index: 0,
            }],
            index: 0,
        }
    }

    pub fn select(&mut self, v: i32) {
        let max = (self.routines.len() - 1) as f32;
        self.index = (map_midi(v, 0.0, max) as usize).min(self.routines.len() - 1);
    }

    pub fn on_beat(&mut self) {
        if self.routines[self.index].on_beat() {
            // pick another routine at random (the sketch never picks the last one)
            self.index = random(0.0, (self.routines.len() - 1) as f32) as usize;
        }
    }

    pub fn reset(&mut self) {
        for r in &mut self.routines {
            r.index = 0;
        }
    }

    fn positions(&self) -> &Pose {
        self.routines[self.index].get()
    }
}

// ---------------------------------------------------------------------------------------------
// Figures

struct StickFigure {
    pos: Vec2,
    position_animation_progress: f32,
    beat_animation_progress: f32,
    current: Pose,
    target: Pose,
    head_color: PColor,
    ani_inc: f32,
}

impl StickFigure {
    const ORIGIN_POS: [(f32, f32); NUM_PARTS] = [
        (0.0, -100.0),
        (0.0, -40.0),
        (0.0, 40.0),
        (-60.0, 20.0),
        (60.0, 20.0),
        (-40.0, 140.0),
        (40.0, 140.0),
        (-30.0, 10.0),
        (30.0, -10.0),
        (-30.0, 90.0),
        (30.0, 90.0),
    ];
    /// Neck, torso, upper arms, lower arms, thighs, shins (pairs of part indices).
    fn origin_pose() -> Pose {
        Self::ORIGIN_POS.map(|(x, y)| vec2(x, y))
    }

    const LIMBS: [(usize, usize); 10] = [
        (0, 1),
        (1, 2),
        (1, 7),
        (1, 8),
        (7, 3),
        (8, 4),
        (2, 9),
        (2, 10),
        (9, 5),
        (10, 6),
    ];
    const BODY_THICKNESS: f32 = 5.0;
    const HEAD_SIZE: f32 = 65.0;

    fn new(w: f32, h: f32, frame_rate: f32, dance: &DanceRoutines) -> Self {
        let mut f = StickFigure {
            pos: vec2(w / 2.0, h / 2.0),
            position_animation_progress: 0.0,
            beat_animation_progress: 0.0,
            current: Self::origin_pose(),
            target: Self::origin_pose(),
            head_color: PColor::rgb(random(0.0, 255.0), random(0.0, 255.0), random(0.0, 255.0)),
            ani_inc: 1.2 / frame_rate,
        };
        f.dance_move(dance);
        f
    }

    fn dance_move(&mut self, dance: &DanceRoutines) {
        self.target = *dance.positions();
        self.beat_animation_progress = 0.0;
    }

    fn update(&mut self) {
        if self.beat_animation_progress < 1.0 {
            // In the sketch previousPos and currentPos end up as the same array, so each frame
            // eases from where the part is now towards the target. Same here.
            let t = ease_in_out_quint(self.beat_animation_progress);
            for j in 0..NUM_PARTS {
                self.current[j] = self.current[j].lerp(self.target[j], t);
            }
            self.beat_animation_progress += self.ani_inc;
        }
    }

    fn draw(&mut self, draw: &Draw, ctx: &FrameCtx) {
        self.update();
        let d = draw.x_y(self.pos.x, self.pos.y);
        let stroke = if ctx.black_or_white {
            PColor::gray(10.0, 255.0)
        } else {
            PColor::gray(240.0, 255.0)
        }
        .lin();
        let bounce = ctx.pulse * 10.0;
        for (k, (a, b)) in Self::LIMBS.iter().enumerate() {
            let p1 = self.current[*a];
            let p2 = self.current[*b];
            // the shins keep the feet planted while everything else bounces
            let y2 = if k > 7 { p2.y } else { p2.y + bounce };
            d.line()
                .start(vec2(p1.x, p1.y + bounce))
                .end(vec2(p2.x, y2))
                .weight(Self::BODY_THICKNESS)
                .caps_round()
                .color(stroke);
        }
        let head = self.current[0];
        d.ellipse()
            .xy(vec2(head.x, head.y + bounce))
            .radius(Self::HEAD_SIZE / 2.0)
            .color(self.head_color.lin())
            .stroke(stroke)
            .stroke_weight(Self::BODY_THICKNESS);
    }
}

fn ease_in_out_quint(t: f32) -> f32 {
    let mut t = t * 2.0;
    if t < 1.0 {
        return 0.5 * t.powi(5);
    }
    t -= 2.0;
    0.5 * (t.powi(5) + 2.0)
}

// ---------------------------------------------------------------------------------------------
// Formations

/// Grid that starts in the centre and works its way out (121 slots).
fn grid(w: f32, h: f32) -> Vec<Vec2> {
    let (columns, rows) = (5, 5);
    let (x_inc, y_inc) = (200, 400);
    let pos_x = (w / 2.0) as i32;
    let origin_y = (h / 2.0) as i32;
    let mut out = Vec::with_capacity(MAX_CROWD_SIZE);
    let x_row = |out: &mut Vec<Vec2>, y: i32| {
        out.push(vec2(pos_x as f32, y as f32));
        let mut xs = x_inc;
        for _ in 0..rows {
            out.push(vec2((pos_x + xs) as f32, y as f32));
            out.push(vec2((pos_x - xs) as f32, y as f32));
            xs += x_inc;
        }
    };
    x_row(&mut out, origin_y);
    let mut ys = y_inc;
    for _ in 0..columns {
        x_row(&mut out, origin_y - ys);
        x_row(&mut out, origin_y + ys);
        ys += y_inc;
    }
    out
}

fn formations(w: f32, h: f32) -> Vec<Vec<Vec2>> {
    let n = MAX_CROWD_SIZE;
    let centre = vec2(w / 2.0, h / 2.0);

    // One: regular grid
    let one = grid(w, h);

    // Two: spiral
    let mut two = Vec::with_capacity(n);
    let (mut angle, mut angle_inc, mut radius) = (0.0f32, 0.6f32, 150.0f32);
    for _ in 0..n {
        two.push(centre + vec2(angle.sin() * radius, angle.cos() * radius));
        angle += angle_inc;
        angle_inc -= 0.01;
        radius += 50.0;
    }

    // Three: jittered grid
    let three = grid(w, h)
        .into_iter()
        .map(|p| {
            let (x, y) = (p.x as i32 as f32, p.y as i32 as f32);
            vec2(
                random(x - 100.0, x + 100.0).trunc(),
                random(y - 100.0, y + 100.0).trunc(),
            )
        })
        .collect();

    // Four: concentric rings
    let ring_sizes = [10, 14, 22, 33, 40];
    let mut four = Vec::with_capacity(n);
    let radius_inc = 280.0;
    let mut radius = radius_inc;
    let mut angle = 0.0f32;
    let mut ring_index = 0;
    let mut num_in_ring = ring_sizes[0];
    let mut angle_inc = TAU / num_in_ring as f32;
    let mut ring_count = 0;
    for _ in 0..n {
        four.push(centre + vec2(angle.sin() * radius, angle.cos() * radius));
        angle += angle_inc;
        ring_count += 1;
        if ring_count > num_in_ring {
            radius += radius_inc;
            angle_inc /= 1.5;
            ring_index = (ring_index + 1).min(ring_sizes.len() - 1);
            num_in_ring = ring_sizes[ring_index];
            ring_count = 0;
        }
    }
    four[0] = centre;

    vec![one, two, three, four]
}

// ---------------------------------------------------------------------------------------------

/// The stick figure overlay (`VisStickFigures` + `StickFigures`).
pub struct VisStickFigures {
    pub on: bool,
    figures: Vec<StickFigure>,
    formations: Vec<Vec<Vec2>>,
    formation_index: usize,
    crowd_size: usize,
    scaling: f32,
    prev_knob_value: i32,
}

impl VisStickFigures {
    /// Speed of moving between formations.
    const A_INC: f32 = 0.001;

    pub fn new(w: f32, h: f32, frame_rate: f32, dance: &DanceRoutines) -> Self {
        VisStickFigures {
            on: false,
            figures: (0..MAX_CROWD_SIZE)
                .map(|_| StickFigure::new(w, h, frame_rate, dance))
                .collect(),
            formations: formations(w, h),
            formation_index: 0,
            crowd_size: 1,
            scaling: 1.0,
            prev_knob_value: 0,
        }
    }

    pub fn toggle(&mut self) {
        self.on = !self.on;
    }

    /// "Rave mode": the knob direction adds or removes a figure.
    pub fn set_crowd_size(&mut self, v: i32) {
        if v > self.prev_knob_value {
            self.crowd_size = (self.crowd_size + 1).min(MAX_CROWD_SIZE);
        } else {
            self.crowd_size = (self.crowd_size - 1).max(1);
        }
        self.prev_knob_value = v;
    }

    pub fn set_scale(&mut self, v: i32) {
        self.scaling = map_midi(v.clamp(0, MIDI_MAX), 0.1, 2.0);
    }

    pub fn next_position_set(&mut self) {
        self.formation_index = (self.formation_index + 1) % self.formations.len();
        for f in self.figures.iter_mut().take(self.crowd_size) {
            f.position_animation_progress = 0.0;
        }
    }

    pub fn on_beat(&mut self, dance: &DanceRoutines) {
        for f in self.figures.iter_mut().take(self.crowd_size) {
            f.dance_move(dance);
        }
    }

    pub fn draw(&mut self, draw: &Draw, ctx: &FrameCtx) {
        if !self.on {
            return;
        }
        let (w, h) = (ctx.t.width, ctx.t.height);
        let d = draw
            .translate(vec3(w / 2.0, h / 2.0, 0.0))
            .scale(self.scaling)
            .translate(vec3(-w / 2.0, -h / 2.0, 0.0));
        let targets = &self.formations[self.formation_index];
        for (i, f) in self.figures.iter_mut().take(self.crowd_size).enumerate() {
            if f.position_animation_progress < 1.0 {
                let target = targets.get(i).copied().unwrap_or(f.pos);
                f.pos = vec2(
                    lerp(f.pos.x, target.x, f.position_animation_progress),
                    lerp(f.pos.y, target.y, f.position_animation_progress),
                );
                f.position_animation_progress += Self::A_INC;
            }
            f.draw(&d, ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_has_121_unique_slots() {
        let g = grid(1200.0, 1200.0);
        assert_eq!(g.len(), MAX_CROWD_SIZE);
        assert_eq!(g[0], vec2(600.0, 600.0));
        let f = formations(1200.0, 1200.0);
        assert!(f.iter().all(|s| s.len() >= MAX_CROWD_SIZE));
    }

    #[test]
    fn loads_dance_moves() {
        let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data");
        let mut d = DanceRoutines::load(&data).unwrap();
        assert!(!d.routines.is_empty());
        for _ in 0..50 {
            d.on_beat();
        }
    }
}
