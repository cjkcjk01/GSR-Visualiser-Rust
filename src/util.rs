//! Small helpers that mirror Processing built-ins (`map`, `constrain`, `noise`, HSB colours, ...).

use nannou::color::LinSrgba;

/// Upper end of the MIDI value range, used throughout as in the sketch.
pub const MIDI_MAX: i32 = 127;

/// Processing's `map()`: re-maps a value from one range to another (no clamping).
pub fn map(v: f32, a0: f32, a1: f32, b0: f32, b1: f32) -> f32 {
    b0 + (b1 - b0) * ((v - a0) / (a1 - a0))
}

/// `map()` for the very common "MIDI value to range" case.
pub fn map_midi(v: i32, b0: f32, b1: f32) -> f32 {
    map(v as f32, 0.0, MIDI_MAX as f32, b0, b1)
}

pub fn constrain(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

/// Processing's `round()` (rounds half up, like `Math.round`).
pub fn pround(v: f32) -> i32 {
    (v + 0.5).floor() as i32
}

/// Processing's `random(lo, hi)`.
pub fn random(lo: f32, hi: f32) -> f32 {
    if hi <= lo {
        return lo;
    }
    lo + rand::random::<f32>() * (hi - lo)
}

/// A colour stored the way Processing stores it: raw 0-255 channel values with no gamma handling.
///
/// All of our offscreen layers are plain `Rgba8Unorm` textures, so we hand nannou "linear" colours
/// whose values are simply the raw channels. That way nothing gets gamma-converted on the way in
/// and the result looks the same as in Processing; the final present pass handles the conversion
/// to the window's colour space.
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct PColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl PColor {
    pub const fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        PColor {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }
    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        PColor {
            r: r / 255.0,
            g: g / 255.0,
            b: b / 255.0,
            a: 1.0,
        }
    }
    /// Greyscale like Processing's `fill(gray)` / `fill(gray, alpha)`.
    pub fn gray(v: f32, a: f32) -> Self {
        PColor {
            r: v / 255.0,
            g: v / 255.0,
            b: v / 255.0,
            a: a / 255.0,
        }
    }
    pub fn with_alpha(self, a255: f32) -> Self {
        PColor {
            a: (a255 / 255.0).clamp(0.0, 1.0),
            ..self
        }
    }
    pub fn lin(self) -> LinSrgba {
        LinSrgba::new(self.r, self.g, self.b, self.a)
    }
    /// Parse a web colour like `#ff8800`.
    pub fn from_hex(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches('#');
        if s.len() != 6 {
            return None;
        }
        let v = u32::from_str_radix(s, 16).ok()?;
        Some(PColor::rgba8((v >> 16) as u8, (v >> 8) as u8, v as u8, 255))
    }
    /// Colour from HSB components already normalised to 0..1, as Processing's `colorMode(HSB, ...)`.
    pub fn hsb(h: f32, s: f32, b: f32) -> Self {
        let (r, g, bl) = hsb_to_rgb(h, s, b);
        PColor {
            r,
            g,
            b: bl,
            a: 1.0,
        }
    }
}

/// HSB (all 0..1) to RGB (0..1), same algorithm Processing uses.
pub fn hsb_to_rgb(h: f32, s: f32, v: f32) -> (f32, f32, f32) {
    let h = h.clamp(0.0, 1.0);
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    if s == 0.0 {
        return (v, v, v);
    }
    let which = (h - h.floor()) * 6.0;
    let f = which - which.floor();
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match which as i32 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    }
}

/// Port of Processing's `noise()` (1D/2D/3D value-noise with 4 octaves and 0.5 falloff).
pub struct PNoise {
    perlin: Vec<f32>,
    cos_table: Vec<f32>,
}

const PERLIN_YWRAPB: u32 = 4;
const PERLIN_YWRAP: usize = 1 << PERLIN_YWRAPB;
const PERLIN_ZWRAPB: u32 = 8;
const PERLIN_ZWRAP: usize = 1 << PERLIN_ZWRAPB;
const PERLIN_SIZE: usize = 4095;
const PERLIN_OCTAVES: usize = 4;
const PERLIN_AMP_FALLOFF: f32 = 0.5;
const SINCOS_LENGTH: usize = 720;
const PERLIN_TWOPI: usize = SINCOS_LENGTH;
const PERLIN_PI: usize = PERLIN_TWOPI >> 1;

impl PNoise {
    pub fn new() -> Self {
        let perlin = (0..=PERLIN_SIZE).map(|_| rand::random::<f32>()).collect();
        let cos_table = (0..SINCOS_LENGTH)
            .map(|i| ((i as f32) * std::f32::consts::PI / 360.0).cos())
            .collect();
        PNoise { perlin, cos_table }
    }

    fn noise_fsc(&self, i: f32) -> f32 {
        let idx = ((i * PERLIN_PI as f32) as usize) % PERLIN_TWOPI;
        0.5 * (1.0 - self.cos_table[idx])
    }

    pub fn noise(&self, x: f32, y: f32, z: f32) -> f32 {
        let (x, y, z) = (x.abs(), y.abs(), z.abs());
        let mut xi = x as usize;
        let mut yi = y as usize;
        let mut zi = z as usize;
        let mut xf = x - xi as f32;
        let mut yf = y - yi as f32;
        let mut zf = z - zi as f32;
        let mut r = 0.0;
        let mut ampl = 0.5;
        for _ in 0..PERLIN_OCTAVES {
            let mut of = xi + (yi << PERLIN_YWRAPB) + (zi << PERLIN_ZWRAPB);
            let rxf = self.noise_fsc(xf);
            let ryf = self.noise_fsc(yf);
            let p = |i: usize| self.perlin[i & PERLIN_SIZE];
            let mut n1 = p(of);
            n1 += rxf * (p(of + 1) - n1);
            let mut n2 = p(of + PERLIN_YWRAP);
            n2 += rxf * (p(of + PERLIN_YWRAP + 1) - n2);
            n1 += ryf * (n2 - n1);
            of += PERLIN_ZWRAP;
            let mut n2b = p(of);
            n2b += rxf * (p(of + 1) - n2b);
            let mut n3 = p(of + PERLIN_YWRAP);
            n3 += rxf * (p(of + PERLIN_YWRAP + 1) - n3);
            n2b += ryf * (n3 - n2b);
            n1 += self.noise_fsc(zf) * (n2b - n1);
            r += n1 * ampl;
            ampl *= PERLIN_AMP_FALLOFF;
            xi <<= 1;
            xf *= 2.0;
            yi <<= 1;
            yf *= 2.0;
            zi <<= 1;
            zf *= 2.0;
            if xf >= 1.0 {
                xi += 1;
                xf -= 1.0;
            }
            if yf >= 1.0 {
                yi += 1;
                yf -= 1.0;
            }
            if zf >= 1.0 {
                zi += 1;
                zf -= 1.0;
            }
        }
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_matches_processing() {
        assert_eq!(map(64.0, 0.0, 128.0, 0.0, 1.0), 0.5);
        assert_eq!(map_midi(127, -5.0, 5.0), 5.0);
        assert_eq!(map_midi(0, -5.0, 5.0), -5.0);
    }

    #[test]
    fn hex_colours() {
        let c = PColor::from_hex("#FF8000").unwrap();
        assert_eq!((c.r, c.b), (1.0, 0.0));
        assert!((c.g - 128.0 / 255.0).abs() < 1e-6);
    }

    #[test]
    fn hsb_primaries() {
        assert_eq!(hsb_to_rgb(0.0, 1.0, 1.0), (1.0, 0.0, 0.0));
        let (r, g, b) = hsb_to_rgb(1.0 / 3.0, 1.0, 1.0);
        assert!(r < 1e-5 && (g - 1.0).abs() < 1e-5 && b < 1e-5);
    }

    #[test]
    fn noise_in_range() {
        let n = PNoise::new();
        for i in 0..1000 {
            let v = n.noise(i as f32 * 0.03, 0.0, 0.0);
            assert!((0.0..=1.0).contains(&v));
        }
    }
}
