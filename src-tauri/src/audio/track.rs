use super::types::{PlaybackMode, TrackSnapshot, TrackState};

pub const MAX_RECORDING_SECONDS: usize = 90;
pub const CROSSFADE_FRAMES: usize = 128; // ~2.9ms at 44.1kHz, eliminates clicks

#[derive(Clone)]
pub struct LoopTrack {
    pub id: usize,
    pub state: TrackState,
    pub mode: PlaybackMode,
    pub buffer: Vec<f32>, // interleaved stereo [L, R, L, R...]
    pub length_frames: usize,
    pub playhead: usize, // current frame index (0..length_frames)
    pub volume: f32,     // 0.0 to 1.5
    pub is_muted: bool,
    pub is_solo: bool,
    pub is_paused: bool,
    pub undo_buffer: Option<Vec<f32>>,
    pub peak_level: f32,
    sample_rate: u32,
}

impl LoopTrack {
    pub fn new(id: usize, sample_rate: u32) -> Self {
        let max_samples = sample_rate as usize * 2 * MAX_RECORDING_SECONDS;
        let mut buffer = Vec::with_capacity(max_samples);
        buffer.resize(0, 0.0);

        Self {
            id,
            state: TrackState::Empty,
            mode: PlaybackMode::Play,
            buffer,
            length_frames: 0,
            playhead: 0,
            volume: 1.0,
            is_muted: false,
            is_solo: false,
            is_paused: false,
            undo_buffer: None,
            peak_level: 0.0,
            sample_rate,
        }
    }

    pub fn start_recording(&mut self) {
        self.buffer.clear();
        self.length_frames = 0;
        self.playhead = 0;
        self.undo_buffer = None;
        self.state = TrackState::Recording;
    }

    pub fn start_overdub(&mut self) {
        if self.length_frames > 0 {
            self.undo_buffer = Some(self.buffer.clone());
            self.state = TrackState::Overdubbing;
        }
    }

    pub fn stop_overdub(&mut self) {
        if self.state == TrackState::Overdubbing {
            self.state = TrackState::Playing;
        }
    }

    pub fn finish_recording(&mut self) {
        if self.state == TrackState::Recording && self.length_frames > 0 {
            self.apply_seam_crossfade();
            self.playhead = 0;
            self.state = TrackState::Playing;
        } else if self.length_frames == 0 {
            self.state = TrackState::Empty;
        }
    }

    pub fn undo(&mut self) {
        if let Some(prev) = self.undo_buffer.take() {
            self.buffer = prev;
            self.length_frames = self.buffer.len() / 2;
            if self.playhead >= self.length_frames && self.length_frames > 0 {
                self.playhead = 0;
            }
        }
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.length_frames = 0;
        self.playhead = 0;
        self.undo_buffer = None;
        self.peak_level = 0.0;
        self.state = TrackState::Empty;
    }

    /// Record a single stereo frame (left, right) during initial record
    pub fn record_frame(&mut self, left: f32, right: f32) {
        let max_frames = self.sample_rate as usize * MAX_RECORDING_SECONDS;
        if self.length_frames < max_frames {
            self.buffer.push(left);
            self.buffer.push(right);
            self.length_frames += 1;
            let peak = left.abs().max(right.abs());
            self.update_peak(peak);
        } else {
            self.finish_recording();
        }
    }

    /// Mix incoming mic frame into existing buffer at playhead position during overdub
    pub fn overdub_frame(&mut self, left: f32, right: f32) {
        if self.length_frames == 0 {
            return;
        }
        let idx = self.playhead * 2;
        if idx + 1 < self.buffer.len() {
            let existing_l = self.buffer[idx];
            let existing_r = self.buffer[idx + 1];

            // Soft saturation / clamping to prevent harsh digital clipping on overdubs
            let new_l = (existing_l + left).clamp(-1.0, 1.0);
            let new_r = (existing_r + right).clamp(-1.0, 1.0);

            self.buffer[idx] = new_l;
            self.buffer[idx + 1] = new_r;

            let peak = new_l.abs().max(new_r.abs());
            self.update_peak(peak);
        }
    }

    /// Read next output frame from this track, advance playhead, and report if it wrapped
    pub fn read_and_advance(&mut self, is_any_solo: bool) -> (f32, f32, bool) {
        if self.length_frames == 0 || self.state == TrackState::Empty {
            return (0.0, 0.0, false);
        }

        // If paused (held), playhead does not advance and output is muted
        if self.is_paused {
            return (0.0, 0.0, false);
        }

        let idx = self.playhead * 2;
        let (raw_l, raw_r) = if idx + 1 < self.buffer.len() {
            (self.buffer[idx], self.buffer[idx + 1])
        } else {
            (0.0, 0.0)
        };

        // Advance playhead and detect seam wrap / one-shot finish
        self.playhead += 1;
        let wrapped = if self.playhead >= self.length_frames {
            self.playhead = 0;
            if self.mode == PlaybackMode::TotalMpc || self.mode == PlaybackMode::PartialMpc {
                // In Total MPC and Partial MPC, play once to the end and stop!
                self.is_paused = true;
            }
            true
        } else {
            false
        };

        // Apply mute / solo logic
        let audible = if is_any_solo {
            self.is_solo && !self.is_muted
        } else {
            !self.is_muted && self.state != TrackState::Muted
        };

        let peak = raw_l.abs().max(raw_r.abs()) * self.volume;
        self.update_peak(peak);

        let out = if audible {
            (raw_l * self.volume, raw_r * self.volume)
        } else {
            (0.0, 0.0)
        };

        (out.0, out.1, wrapped)
    }

    /// Micro-crossfade the last CROSSFADE_FRAMES into the start to prevent clicks
    fn apply_seam_crossfade(&mut self) {
        let n = CROSSFADE_FRAMES.min(self.length_frames / 4);
        if n == 0 {
            return;
        }

        for i in 0..n {
            let t = i as f32 / n as f32; // 0.0 to 1.0
            let start_idx = i * 2;
            let end_idx = (self.length_frames - n + i) * 2;

            if end_idx + 1 < self.buffer.len() && start_idx + 1 < self.buffer.len() {
                let start_l = self.buffer[start_idx];
                let start_r = self.buffer[start_idx + 1];
                let end_l = self.buffer[end_idx];
                let end_r = self.buffer[end_idx + 1];

                // Blend start: fade in from end
                self.buffer[start_idx] = start_l * t + end_l * (1.0 - t);
                self.buffer[start_idx + 1] = start_r * t + end_r * (1.0 - t);

                // Blend end: fade out into start
                self.buffer[end_idx] = end_l * (1.0 - t) + start_l * t;
                self.buffer[end_idx + 1] = end_r * (1.0 - t) + start_r * t;
            }
        }
    }

    fn update_peak(&mut self, peak: f32) {
        if peak > self.peak_level {
            self.peak_level = peak;
        } else {
            self.peak_level = self.peak_level * 0.92; // smooth decay
        }
    }

    pub fn snapshot(&self) -> TrackSnapshot {
        let playhead_norm = if self.length_frames > 0 {
            (self.playhead as f32 / self.length_frames as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let length_seconds = self.length_frames as f32 / self.sample_rate as f32;

        TrackSnapshot {
            id: self.id,
            state: self.state,
            mode: self.mode,
            playhead_norm,
            length_seconds,
            volume: self.volume,
            is_muted: self.is_muted || self.state == TrackState::Muted,
            is_solo: self.is_solo,
            peak_level: self.peak_level.clamp(0.0, 1.0),
            has_undo: self.undo_buffer.is_some(),
        }
    }
}
