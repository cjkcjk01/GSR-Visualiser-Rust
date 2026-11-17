//! FFT analysis, a port of the parts of Minim's `FFT` the sketch uses plus the `FFTAnalysis` class.

use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

/// Minim-compatible FFT: Hamming window, un-normalised magnitude spectrum of `time_size/2 + 1` bands.
pub struct Fft {
    time_size: usize,
    sample_rate: f32,
    window: Vec<f32>,
    plan: Arc<dyn rustfft::Fft<f32>>,
    buf: Vec<Complex<f32>>,
    spectrum: Vec<f32>,
}

impl Fft {
    pub fn new(time_size: usize, sample_rate: f32) -> Self {
        let window = (0..time_size)
            .map(|i| {
                0.54 - 0.46 * (std::f32::consts::TAU * i as f32 / (time_size as f32 - 1.0)).cos()
            })
            .collect();
        let plan = FftPlanner::new().plan_fft_forward(time_size);
        Fft {
            time_size,
            sample_rate,
            window,
            plan,
            buf: vec![Complex::default(); time_size],
            spectrum: vec![0.0; time_size / 2 + 1],
        }
    }

    pub fn forward(&mut self, samples: &[f32]) {
        for i in 0..self.time_size {
            let s = samples.get(i).copied().unwrap_or(0.0);
            self.buf[i] = Complex::new(s * self.window[i], 0.0);
        }
        self.plan.process(&mut self.buf);
        for (i, s) in self.spectrum.iter_mut().enumerate() {
            *s = self.buf[i].norm();
        }
    }

    pub fn spec_size(&self) -> usize {
        self.spectrum.len()
    }

    pub fn band_width(&self) -> f32 {
        (2.0 / self.time_size as f32) * (self.sample_rate / 2.0)
    }

    pub fn freq_to_index(&self, freq: f32) -> usize {
        let bw = self.band_width();
        if freq < bw / 2.0 {
            return 0;
        }
        if freq > self.sample_rate / 2.0 - bw / 2.0 {
            return self.spectrum.len() - 1;
        }
        let fraction = freq / self.sample_rate;
        ((self.time_size as f32 * fraction).round() as usize).min(self.spectrum.len() - 1)
    }

    /// Amplitude of a band by index (`getBand`).
    pub fn get_band(&self, i: usize) -> f32 {
        self.spectrum.get(i).copied().unwrap_or(0.0)
    }

    /// Amplitude of the band containing a frequency in Hz (`getFreq`).
    pub fn get_freq(&self, freq: f32) -> f32 {
        self.get_band(self.freq_to_index(freq))
    }

    /// Average amplitude of the bands between two frequencies (`calcAvg`).
    pub fn calc_avg(&self, low: f32, high: f32) -> f32 {
        let lo = self.freq_to_index(low);
        let hi = self.freq_to_index(high);
        if hi < lo {
            return 0.0;
        }
        self.spectrum[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32
    }
}

/// Splits the spectrum into three bands that roughly match Traktor's mixer EQ (Z-ISO mode) and
/// keeps normalised and slowly decaying versions of each.
pub struct FftAnalysis {
    pub fft: Fft,
    /// Level of signal in each band (low, mid, high).
    pub traktor_eq: [f32; 3],
    /// Highest level seen in each band, used for normalisation.
    pub max_levels: [f32; 3],
    /// `traktor_eq / max_levels * 100`.
    pub normalised_levels: [f32; 3],
    /// Like `normalised_levels` but decays slowly after a peak.
    pub slow_levels: [f32; 3],
    pub decay_factor: f32,
    pub lowest: f32,
    pub crossover1: f32,
    pub crossover2: f32,
    pub highest: f32,
    pub noise_floor: f32,
    pub spec_size: usize,
}

impl FftAnalysis {
    pub fn new(buffer_size: usize, sample_rate: f32) -> Self {
        let fft = Fft::new(buffer_size, sample_rate);
        let spec_size = fft.spec_size();
        let noise_floor = 0.1;
        FftAnalysis {
            fft,
            traktor_eq: [0.0; 3],
            max_levels: [noise_floor; 3],
            normalised_levels: [0.0; 3],
            slow_levels: [0.0; 3],
            decay_factor: 2.0,
            lowest: 20.0,
            crossover1: 200.0,
            crossover2: 1360.0,
            highest: 16000.0,
            noise_floor,
            spec_size,
        }
    }

    pub fn update(&mut self, samples: &[f32]) {
        self.fft.forward(samples);
        self.traktor_eq[0] = self.fft.calc_avg(self.lowest, self.crossover1);
        self.traktor_eq[1] = self.fft.calc_avg(self.crossover1, self.crossover2);
        self.traktor_eq[2] = self.fft.calc_avg(self.crossover2, self.highest);
        for i in 0..3 {
            if self.traktor_eq[i] < self.noise_floor {
                self.traktor_eq[i] = self.noise_floor;
            }
            if self.traktor_eq[i] > self.max_levels[i] {
                self.max_levels[i] = self.traktor_eq[i];
            }
            let s = self.traktor_eq[i] / self.max_levels[i] * 100.0;
            self.normalised_levels[i] = s;
            if s > self.slow_levels[i] {
                self.slow_levels[i] = s;
            }
            self.slow_levels[i] -= self.decay_factor;
        }
    }

    /// `Visualiser.chooseResponseType()`. Note the original `switch` has no `break`, so both
    /// response types end up reading `normalisedLevels[t]`; that behaviour is kept.
    pub fn choose_response_type(&self, t: usize) -> f32 {
        match t {
            0 | 1 => self.normalised_levels[t],
            _ => 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sine_peaks_in_right_band() {
        let rate = 48_000.0;
        let n = 1024;
        let freq = 1500.0;
        let samples: Vec<f32> = (0..n)
            .map(|i| (std::f32::consts::TAU * freq * i as f32 / rate).sin())
            .collect();
        let mut fft = Fft::new(n, rate);
        fft.forward(&samples);
        let peak = (0..fft.spec_size())
            .max_by(|a, b| fft.get_band(*a).total_cmp(&fft.get_band(*b)))
            .unwrap();
        assert_eq!(peak, fft.freq_to_index(freq));
        // Minim does not normalise: a full scale sine with a Hamming window peaks around N/4.
        assert!(fft.get_band(peak) > 200.0);
    }

    #[test]
    fn analysis_bands() {
        let rate = 48_000.0;
        let samples: Vec<f32> = (0..1024)
            .map(|i| (std::f32::consts::TAU * 100.0 * i as f32 / rate).sin())
            .collect();
        let mut a = FftAnalysis::new(1024, rate);
        a.update(&samples);
        assert!(a.traktor_eq[0] > a.traktor_eq[1]);
        assert!(a.traktor_eq[0] > a.traktor_eq[2]);
        assert!((a.normalised_levels[0] - 100.0).abs() < 1e-3);
    }
}
