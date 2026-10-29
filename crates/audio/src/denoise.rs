//! Taking steady background noise (hiss, hum, room tone) out of a recording when it is
//! re-encoded.
//!
//! The noise is learned first: the recording's quietest 50 ms windows, from the scan, are where
//! nothing but the noise plays, and their spectrum is the *noise print*. Then each track is run
//! through a short-time Fourier transform, and every frequency is turned down by how much of it
//! the noise print explains (spectral subtraction): a bin well above the noise passes as is, a
//! bin at the noise's level goes down by the setting's reduction, never further. The gains are
//! smoothed across neighbouring bins and released slowly over time, which keeps the "musical
//! noise" twitter of plain spectral subtraction down, and they rise at once, so attacks pass.
//!
//! The filter is a `PcmSource` like `Gain`: seeking it lands on the same sample as the source,
//! with enough audio read before that point that a track's first samples are filtered exactly
//! as they would be in the middle of the recording.

use crate::peaks::Loudness;
use crate::source::PcmSource;
use anyhow::Result;
use realfft::num_complex::Complex32;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;
use trackcut_core::export::Denoise;

/// At least this many quiet windows (half a second) to learn the noise from.
const MIN_WINDOWS: usize = 10;
/// Twenty seconds of quiet is plenty, and keeps learning fast on a long recording.
const MAX_WINDOWS: usize = 400;
/// A recording whose quietest moments are louder than this has no stretch of plain noise:
/// whatever is there is the programme, and learning it as noise would eat it.
const QUIETEST_DB: f32 = -30.0;
/// Windows this far above the quietest already hold something besides the noise.
const NEAR_FLOOR_DB: f32 = 6.0;
/// Per hop, how much of a lowered gain is kept while it comes back down (about 50 ms to
/// settle at a 44.1 kHz, 2048-sample frame).
const RELEASE: f32 = 0.6;

/// The noise's average power in each frequency bin, per channel, for frames of `len` samples
/// windowed as `Denoising` windows them.
#[derive(Clone, Debug, PartialEq)]
pub struct NoisePrint {
    pub len: usize,
    pub power: Vec<Vec<f32>>,
}

/// The frame length for windows of `window` samples: the largest power of two that fits in one
/// (2048 at 44.1 and 48 kHz), so a single quiet window is a whole frame.
pub fn frame_len(window: u64) -> usize {
    let mut len = 256usize;
    while (len * 2) as u64 <= window && len < 8192 {
        len *= 2;
    }
    len
}

/// The indices of the quiet windows to learn the noise from, or `None` when the recording has
/// no stretch quiet enough to be noise alone.
pub fn quiet_windows(loudness: &Loudness) -> Option<Vec<usize>> {
    // Digital silence carries no noise to learn, and an empty window reads as -inf.
    let mut audible: Vec<(usize, f32)> =
        loudness.db.iter().copied().enumerate().filter(|(_, db)| db.is_finite() && *db > -90.0).collect();
    if audible.len() < MIN_WINDOWS {
        return None;
    }
    audible.sort_by(|a, b| a.1.total_cmp(&b.1));
    let quietest = audible[0].1;
    if quietest > QUIETEST_DB {
        return None;
    }
    let take = (audible.len() / 20).clamp(MIN_WINDOWS, MAX_WINDOWS);
    let mut picked: Vec<usize> =
        audible.iter().take(take).take_while(|(_, db)| *db <= quietest + NEAR_FLOOR_DB).map(|(i, _)| *i).collect();
    if picked.len() < MIN_WINDOWS {
        return None;
    }
    // In file order: one pass forward through the recording.
    picked.sort_unstable();
    Some(picked)
}

/// The square root of a periodic Hann window: applied before the transform and again after,
/// the two make a Hann window, whose copies a quarter frame apart add up to exactly 2.
fn sqrt_hann(len: usize) -> Vec<f32> {
    (0..len).map(|n| (0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / len as f32).cos()).sqrt()).collect()
}

/// Learns the noise of `src` from its quiet windows. `None` when it has none (see
/// [`quiet_windows`]).
pub fn noise_print(src: &mut dyn PcmSource, loudness: &Loudness) -> Result<Option<NoisePrint>> {
    let Some(windows) = quiet_windows(loudness) else {
        return Ok(None);
    };
    let ch = src.channels();
    let len = frame_len(loudness.window);
    let window = sqrt_hann(len);
    let fft = RealFftPlanner::<f32>::new().plan_fft_forward(len);
    let mut power = vec![vec![0f32; len / 2 + 1]; ch];
    let mut frame = vec![0f32; len];
    let mut spectrum = fft.make_output_vec();
    let mut buf = Vec::with_capacity(len * ch * 2);
    let mut learned = 0usize;
    for w in windows {
        src.seek(w as u64 * loudness.window)?;
        buf.clear();
        while buf.len() < len * ch {
            if !src.read(&mut buf)? {
                break;
            }
        }
        if buf.len() < len * ch {
            continue;
        }
        for (c, bins) in power.iter_mut().enumerate() {
            for (i, x) in frame.iter_mut().enumerate() {
                *x = buf[i * ch + c] * window[i];
            }
            fft.process(&mut frame, &mut spectrum).map_err(|e| anyhow::anyhow!("noise print: {e}"))?;
            for (p, s) in bins.iter_mut().zip(&spectrum) {
                *p += s.norm_sqr();
            }
        }
        learned += 1;
    }
    if learned < MIN_WINDOWS {
        return Ok(None);
    }
    for bins in &mut power {
        bins.iter_mut().for_each(|p| *p /= learned as f32);
    }
    Ok(Some(NoisePrint { len, power }))
}

/// Learns the noise of the recording at `path` (see [`noise_print`]).
pub fn learn(path: &std::path::Path, scan: &crate::Scan) -> Result<Option<NoisePrint>> {
    let mut src = crate::source::open_source(path, scan.mp3.clone().map(Arc::new))?;
    noise_print(src.as_mut(), &scan.loudness)
}

/// One channel's running transform: the last frame of input, the output still being added up,
/// and the gains of the last frame.
struct Channel {
    input: Vec<f32>,
    output: Vec<f32>,
    gains: Vec<f32>,
}

/// `inner` with the noise in `print` taken out, as `setting` says.
pub struct Denoising<'a> {
    inner: &'a mut dyn PcmSource,
    print: &'a NoisePrint,
    floor: f32,
    over: f32,
    hop: usize,
    window: Vec<f32>,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    channels: Vec<Channel>,
    /// Interleaved input not yet a whole hop.
    pending: Vec<f32>,
    /// Output frames still to drop: the pre-roll read before the sought position, and the
    /// transform's own delay.
    skip: usize,
    /// The source has ended; zeros are fed until everything it gave has come out.
    ended: bool,
    /// Frames of zeros still to feed after the end.
    flush: usize,
    frame: Vec<f32>,
    spectrum: Vec<Complex32>,
    scratch_gains: Vec<f32>,
}

impl<'a> Denoising<'a> {
    pub fn new(inner: &'a mut dyn PcmSource, print: &'a NoisePrint, setting: Denoise) -> Self {
        let len = print.len;
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(len);
        let ifft = planner.plan_fft_inverse(len);
        let spectrum = fft.make_output_vec();
        let ch = inner.channels();
        let reduction = setting.reduction_db().unwrap_or(0.0);
        Self {
            inner,
            print,
            floor: 10f32.powf(-reduction / 20.0),
            over: setting.oversubtraction(),
            hop: len / 4,
            window: sqrt_hann(len),
            fft,
            ifft,
            channels: Vec::with_capacity(ch),
            pending: Vec::new(),
            skip: 0,
            ended: false,
            flush: 0,
            frame: vec![0.0; len],
            scratch_gains: vec![1.0; spectrum.len()],
            spectrum,
        }
    }

    /// Samples between a sample going in and coming out.
    fn delay(&self) -> usize {
        self.print.len - self.hop
    }

    /// Read before a sought position: enough that the frames reaching it hold real audio, and
    /// that the released gains have settled as they would have mid-recording.
    fn preroll(&self) -> usize {
        self.print.len * 8
    }

    fn reset(&mut self) {
        let len = self.print.len;
        let bins = len / 2 + 1;
        self.channels = (0..self.inner.channels())
            .map(|_| Channel { input: vec![0.0; len], output: vec![0.0; len], gains: vec![1.0; bins] })
            .collect();
        self.pending.clear();
        self.ended = false;
        self.flush = 0;
    }

    /// Runs one hop of interleaved input through every channel, appending a hop of output.
    fn hop_through(&mut self, input: &[f32], out: &mut Vec<f32>) -> Result<()> {
        let ch = self.channels.len();
        let (len, hop) = (self.print.len, self.hop);
        let scale = 1.0 / (len as f32 * 2.0);
        for c in 0..ch {
            let state = &mut self.channels[c];
            state.input.copy_within(hop.., 0);
            for (i, x) in state.input[len - hop..].iter_mut().enumerate() {
                *x = input[i * ch + c];
            }
            for (f, (x, w)) in self.frame.iter_mut().zip(state.input.iter().zip(&self.window)) {
                *f = x * w;
            }
            self.fft.process(&mut self.frame, &mut self.spectrum).map_err(|e| anyhow::anyhow!("denoise: {e}"))?;

            let noise = &self.print.power[c.min(self.print.power.len() - 1)];
            for (g, (s, n)) in self.scratch_gains.iter_mut().zip(self.spectrum.iter().zip(noise)) {
                let power = s.norm_sqr().max(1e-20);
                *g = (1.0 - self.over * n / power).max(0.0).sqrt().max(self.floor);
            }
            // Across neighbouring bins, then over time: rise at once, come down slowly.
            let bins = self.scratch_gains.len();
            for k in 0..bins {
                let lo = k.saturating_sub(1);
                let hi = (k + 1).min(bins - 1);
                let smooth = self.scratch_gains[lo..=hi].iter().sum::<f32>() / (hi - lo + 1) as f32;
                let prev = state.gains[k];
                state.gains[k] = if smooth >= prev { smooth } else { prev * RELEASE + smooth * (1.0 - RELEASE) };
            }
            for (s, g) in self.spectrum.iter_mut().zip(&state.gains) {
                *s *= *g;
            }
            // A real signal's spectrum has no imaginary part at 0 Hz and at Nyquist.
            self.spectrum[0].im = 0.0;
            self.spectrum[bins - 1].im = 0.0;
            self.ifft.process(&mut self.spectrum, &mut self.frame).map_err(|e| anyhow::anyhow!("denoise: {e}"))?;

            for (o, (x, w)) in state.output.iter_mut().zip(self.frame.iter().zip(&self.window)) {
                *o += x * w * scale;
            }
        }
        let start = out.len();
        out.resize(start + hop * ch, 0.0);
        for c in 0..ch {
            let state = &mut self.channels[c];
            for i in 0..hop {
                out[start + i * ch + c] = state.output[i];
            }
            state.output.copy_within(hop.., 0);
            state.output[len - hop..].iter_mut().for_each(|x| *x = 0.0);
        }
        Ok(())
    }
}

impl PcmSource for Denoising<'_> {
    fn channels(&self) -> usize {
        self.inner.channels()
    }

    fn sample_rate(&self) -> u32 {
        self.inner.sample_rate()
    }

    fn seek(&mut self, frame: u64) -> Result<()> {
        let pre = (self.preroll() as u64).min(frame);
        self.inner.seek(frame - pre)?;
        self.reset();
        self.skip = pre as usize + self.delay();
        Ok(())
    }

    fn read(&mut self, out: &mut Vec<f32>) -> Result<bool> {
        if self.channels.is_empty() {
            self.reset();
            self.skip = self.delay();
        }
        let ch = self.channels.len();
        let hop_samples = self.hop * ch;
        let mut produced = Vec::new();
        while produced.is_empty() {
            if !self.ended && !self.inner.read(&mut self.pending)? {
                self.ended = true;
                self.flush = self.delay() + self.hop;
            }
            if self.ended {
                if self.flush == 0 && self.pending.is_empty() {
                    return Ok(false);
                }
                // Pad to a whole hop, then keep feeding silence until the delay is drained.
                let short = (hop_samples - self.pending.len() % hop_samples) % hop_samples;
                let pad = if short > 0 {
                    short
                } else if self.flush > 0 {
                    hop_samples
                } else {
                    0
                };
                self.pending.resize(self.pending.len() + pad, 0.0);
                self.flush = self.flush.saturating_sub(pad / ch);
            }
            let whole = self.pending.len() / hop_samples * hop_samples;
            let input: Vec<f32> = self.pending.drain(..whole).collect();
            for hop in input.chunks_exact(hop_samples) {
                self.hop_through(hop, &mut produced)?;
            }
            let drop = self.skip.min(produced.len() / ch);
            produced.drain(..drop * ch);
            self.skip -= drop;
            if self.ended && self.flush == 0 && self.pending.is_empty() && produced.is_empty() {
                return Ok(false);
            }
        }
        out.extend_from_slice(&produced);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A source over samples held in memory.
    struct Memory {
        samples: Vec<f32>,
        channels: usize,
        pos: usize,
    }

    impl PcmSource for Memory {
        fn channels(&self) -> usize {
            self.channels
        }

        fn sample_rate(&self) -> u32 {
            44_100
        }

        fn seek(&mut self, frame: u64) -> Result<()> {
            self.pos = (frame as usize * self.channels).min(self.samples.len());
            Ok(())
        }

        fn read(&mut self, out: &mut Vec<f32>) -> Result<bool> {
            if self.pos >= self.samples.len() {
                return Ok(false);
            }
            let end = (self.pos + 1000 * self.channels).min(self.samples.len());
            out.extend_from_slice(&self.samples[self.pos..end]);
            self.pos = end;
            Ok(true)
        }
    }

    /// Deterministic white noise at `amplitude`.
    fn noise(frames: usize, amplitude: f32, seed: u32) -> Vec<f32> {
        let mut x = seed;
        (0..frames)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                (x as f32 / u32::MAX as f32 * 2.0 - 1.0) * amplitude
            })
            .collect()
    }

    fn tone(frames: usize, hz: f32, amplitude: f32) -> Vec<f32> {
        (0..frames).map(|i| (2.0 * std::f32::consts::PI * hz * i as f32 / 44_100.0).sin() * amplitude).collect()
    }

    fn read_range(src: &mut dyn PcmSource, start: u64, frames: usize) -> Vec<f32> {
        src.seek(start).unwrap();
        let mut out = Vec::new();
        while out.len() < frames * src.channels() {
            assert!(src.read(&mut out).unwrap(), "ended early");
        }
        out.truncate(frames * src.channels());
        out
    }

    fn rms(x: &[f32]) -> f32 {
        (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
    }

    fn silent_print(len: usize, channels: usize) -> NoisePrint {
        NoisePrint { len, power: vec![vec![0.0; len / 2 + 1]; channels] }
    }

    #[test]
    fn with_no_noise_learned_the_sound_comes_out_as_it_went_in() {
        let input: Vec<f32> =
            noise(44_100, 0.5, 7).into_iter().zip(tone(44_100, 440.0, 0.3)).map(|(a, b)| a + b).collect();
        let mut src = Memory { samples: input.clone(), channels: 1, pos: 0 };
        let print = silent_print(2048, 1);
        let mut d = Denoising::new(&mut src, &print, Denoise::Strong);
        for start in [0u64, 5_000, 30_000] {
            let out = read_range(&mut d, start, 8_000);
            let want = &input[start as usize..start as usize + 8_000];
            let worst = out.iter().zip(want).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
            assert!(worst < 1e-4, "start {start}: off by {worst}");
        }
    }

    #[test]
    fn the_last_samples_of_the_recording_come_out_too() {
        let input = tone(10_000, 300.0, 0.5);
        let mut src = Memory { samples: input.clone(), channels: 1, pos: 0 };
        let print = silent_print(2048, 1);
        let mut d = Denoising::new(&mut src, &print, Denoise::Light);
        let out = read_range(&mut d, 6_000, 4_000);
        let worst = out.iter().zip(&input[6_000..]).map(|(a, b)| (a - b).abs()).fold(0f32, f32::max);
        assert!(worst < 1e-4, "off by {worst}");
    }

    #[test]
    fn learned_noise_is_turned_down_and_a_tone_over_it_is_kept() {
        let frames = 44_100 * 3;
        let hiss = noise(frames, 0.02, 3);
        // A second of hiss alone, then a loud tone over the same hiss.
        let mut input = hiss.clone();
        let loud = tone(frames, 1000.0, 0.5);
        for i in 44_100..frames {
            input[i] += loud[i];
        }
        let window = 2205u64;
        let db: Vec<f32> = input.chunks(window as usize).map(|w| 20.0 * rms(w).max(1e-9).log10()).collect();
        let loudness = Loudness { window, db };
        let mut src = Memory { samples: input.clone(), channels: 1, pos: 0 };
        let print = noise_print(&mut src, &loudness).unwrap().expect("learned from the hiss");

        let mut d = Denoising::new(&mut src, &print, Denoise::Medium);
        let quiet = read_range(&mut d, 10_000, 20_000);
        let reduced_db = 20.0 * (rms(&quiet) / rms(&input[10_000..30_000])).log10();
        assert!(reduced_db < -9.0, "hiss only down {reduced_db:.1} dB");

        let busy = read_range(&mut d, 60_000, 20_000);
        let kept_db = 20.0 * (rms(&busy) / rms(&input[60_000..80_000])).log10();
        assert!(kept_db > -0.5, "the tone lost {kept_db:.1} dB");
    }

    #[test]
    fn a_recording_without_a_quiet_moment_learns_nothing() {
        let loud = Loudness { window: 2205, db: vec![-12.0; 200] };
        assert_eq!(quiet_windows(&loud), None);
        let silent = Loudness { window: 2205, db: vec![f32::NEG_INFINITY; 200] };
        assert_eq!(quiet_windows(&silent), None, "digital silence is not noise");
    }

    #[test]
    fn only_windows_near_the_quietest_are_learned_from() {
        let mut db = vec![-20.0f32; 400];
        for (i, d) in db.iter_mut().enumerate().take(30) {
            *d = if i % 2 == 0 { -60.0 } else { -45.0 };
        }
        let picked = quiet_windows(&Loudness { window: 2205, db }).unwrap();
        assert_eq!(picked.len(), 15);
        assert!(picked.iter().all(|i| i % 2 == 0 && *i < 30));
    }

    #[test]
    fn frames_fit_inside_one_window() {
        assert_eq!(frame_len(2205), 2048);
        assert_eq!(frame_len(2400), 2048);
        assert_eq!(frame_len(4800), 4096);
        assert_eq!(frame_len(400), 256);
    }
}
