use crossbeam_channel::Receiver;
use std::sync::{Arc, Mutex};

use super::track::LoopTrack;
use super::types::{AudioCommand, LooperSnapshot, PlaybackMode, TrackSnapshot, TrackState};

pub const NUM_TRACKS: usize = 4;

pub struct LooperProcessor {
    pub tracks: Vec<LoopTrack>,
    pub active_track_idx: usize,
    pub master_track_idx: Option<usize>,
    pub is_master_playing: bool,
    pub monitor_mic: bool,
    pub monitor_volume: f32,
    pub input_gain: f32,
    pub mic_peak_level: f32,
    pub sample_rate: u32,
    // Metronome state
    pub metro_enabled: bool,
    pub metro_bpm: f32,
    pub metro_volume: f32,
    pub metro_cue_only: bool,
    pub metro_phase: f64,
    pub metro_beat: u32,
    metro_click_sample: usize,
    metro_click_duration: usize,
    metro_is_accent: bool,
    cmd_rx: Receiver<AudioCommand>,
    snapshot_slot: Arc<Mutex<LooperSnapshot>>,
}

impl LooperProcessor {
    pub fn new(
        sample_rate: u32,
        cmd_rx: Receiver<AudioCommand>,
        snapshot_slot: Arc<Mutex<LooperSnapshot>>,
    ) -> Self {
        let mut tracks = Vec::with_capacity(NUM_TRACKS);
        for id in 0..NUM_TRACKS {
            tracks.push(LoopTrack::new(id, sample_rate));
        }

        Self {
            tracks,
            active_track_idx: 0,
            master_track_idx: None,
            is_master_playing: true,
            monitor_mic: true,
            monitor_volume: 1.0,
            input_gain: 1.0,
            mic_peak_level: 0.0,
            sample_rate,
            metro_enabled: false,
            metro_bpm: 120.0,
            metro_volume: 0.7,
            metro_cue_only: true,
            metro_phase: 0.0,
            metro_beat: 0,
            metro_click_sample: 0,
            metro_click_duration: (sample_rate as f32 * 0.025) as usize, // 25ms percussive click
            metro_is_accent: false,
            cmd_rx,
            snapshot_slot,
        }
    }

    /// Process commands sent from the UI/keyboard
    pub fn handle_commands(&mut self) {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                AudioCommand::SelectTrack(idx) => {
                    if idx < NUM_TRACKS {
                        self.active_track_idx = idx;
                    }
                }
                AudioCommand::TriggerTrack(idx) => {
                    if idx < NUM_TRACKS {
                        self.trigger_track_action(idx);
                    }
                }
                AudioCommand::ToggleMute(idx) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].is_muted = !self.tracks[idx].is_muted;
                    }
                }
                AudioCommand::SetTrackMute(idx, muted) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].is_muted = muted;
                    }
                }
                AudioCommand::SetTrackPaused(idx, paused) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].is_paused = paused;
                    }
                }
                AudioCommand::RetriggerTrack(idx) => {
                    if idx < NUM_TRACKS {
                        let t = &mut self.tracks[idx];
                        if t.length_frames > 0 {
                            t.playhead = 0;
                            t.is_paused = false;
                            t.is_muted = false;
                            if t.state != TrackState::Recording {
                                t.state = TrackState::Playing;
                            }
                        }
                    }
                }
                AudioCommand::SetTrackMode(idx, mode) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].mode = mode;
                        // When switching to Play (Loop) mode, automatically resume playback
                        if mode == PlaybackMode::Play {
                            self.tracks[idx].is_paused = false;
                        }
                    }
                }
                AudioCommand::ToggleSolo(idx) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].is_solo = !self.tracks[idx].is_solo;
                    }
                }
                AudioCommand::SetTrackVolume(idx, vol) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].volume = vol.clamp(0.0, 1.5);
                    }
                }
                AudioCommand::ClearTrack(idx) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].clear();
                        if self.master_track_idx == Some(idx) {
                            // Find another playing track to be master, or reset master
                            self.master_track_idx = self
                                .tracks
                                .iter()
                                .position(|t| t.length_frames > 0 && t.state != TrackState::Empty);
                        }
                    }
                }
                AudioCommand::UndoTrack(idx) => {
                    if idx < NUM_TRACKS {
                        self.tracks[idx].undo();
                    }
                }
                AudioCommand::MasterPlayStop => {
                    self.is_master_playing = !self.is_master_playing;
                }
                AudioCommand::ClearAll => {
                    for t in &mut self.tracks {
                        t.clear();
                    }
                    self.master_track_idx = None;
                    self.is_master_playing = true;
                }
                AudioCommand::SetMonitorMic(enable) => {
                    self.monitor_mic = enable;
                }
                AudioCommand::SetMonitorVolume(vol) => {
                    self.monitor_volume = vol.clamp(0.0, 1.5);
                }
                AudioCommand::SetInputGain(gain) => {
                    self.input_gain = gain.clamp(0.0, 2.0);
                }
                AudioCommand::SetMetroEnabled(enable) => {
                    self.metro_enabled = enable;
                    if enable {
                        self.metro_phase = 0.0;
                        self.metro_beat = 0;
                    }
                }
                AudioCommand::SetMetroBpm(bpm) => {
                    self.metro_bpm = bpm.clamp(40.0, 260.0);
                }
                AudioCommand::SetMetroVolume(vol) => {
                    self.metro_volume = vol.clamp(0.0, 1.5);
                }
                AudioCommand::SetMetroCueOnly(cue_only) => {
                    self.metro_cue_only = cue_only;
                }
            }
        }
    }

    /// Boss RC-505 style contextual button logic
    fn trigger_track_action(&mut self, idx: usize) {
        let master_active = self
            .master_track_idx
            .and_then(|m_idx| self.tracks.get(m_idx))
            .map(|m| m.length_frames > 0 && self.is_master_playing)
            .unwrap_or(false);

        let track = &mut self.tracks[idx];

        match track.state {
            TrackState::Empty => {
                // If master track already exists and is playing, queue recording to sync with loop start!
                if master_active && self.master_track_idx != Some(idx) {
                    track.state = TrackState::QueuedRecord;
                } else {
                    track.start_recording();
                    if self.master_track_idx.is_none() {
                        self.master_track_idx = Some(idx);
                    }
                }
            }
            TrackState::QueuedRecord => {
                // Cancel queued record
                track.state = TrackState::Empty;
            }
            TrackState::Recording => {
                // Stop recording -> play
                track.finish_recording();
            }
            TrackState::Playing => {
                // If the track was paused (e.g. from MPC mode or pause on hold), resume it first!
                if track.is_paused {
                    track.is_paused = false;
                } else {
                    // Play -> Overdub
                    track.start_overdub();
                }
            }
            TrackState::Overdubbing => {
                // Overdub -> Play
                track.stop_overdub();
            }
            TrackState::Muted => {
                // Unmute
                track.is_muted = false;
                track.state = TrackState::Playing;
            }
        }
    }

    /// Process a block of stereo frames:
    /// Ingests input mic frames, records/overdubs to tracks, and writes mixed output frames.
    pub fn process_block(&mut self, input_frames: &[(f32, f32)], output_frames: &mut [(f32, f32)]) {
        self.handle_commands();

        let is_any_solo = self.tracks.iter().any(|t| t.is_solo);

        let master_idx = self.master_track_idx;
        let frame_count = input_frames.len().min(output_frames.len());

        for i in 0..frame_count {
            let (raw_in_l, raw_in_r) = input_frames[i];
            let mic_l = raw_in_l * self.input_gain;
            let mic_r = raw_in_r * self.input_gain;

            let in_peak = mic_l.abs().max(mic_r.abs());
            if in_peak > self.mic_peak_level {
                self.mic_peak_level = in_peak;
            } else {
                self.mic_peak_level *= 0.95;
            }

            // Feed mic into recording / overdubbing tracks
            for t in &mut self.tracks {
                match t.state {
                    TrackState::Recording => {
                        t.record_frame(mic_l, mic_r);
                    }
                    TrackState::Overdubbing => {
                        t.overdub_frame(mic_l, mic_r);
                    }
                    _ => {}
                }
            }

            // Sum audio output from playing tracks
            let mut mix_l = 0.0f32;
            let mut mix_r = 0.0f32;
            let mut master_wrapped = false;

            if self.is_master_playing {
                for (idx, t) in self.tracks.iter_mut().enumerate() {
                    if t.state == TrackState::Playing || t.state == TrackState::Overdubbing {
                        let (track_l, track_r, wrapped) = t.read_and_advance(is_any_solo);
                        mix_l += track_l;
                        mix_r += track_r;
                        if Some(idx) == master_idx && wrapped {
                            master_wrapped = true;
                        }
                    }
                }
            }

            // If master loop wrapped, activate queued track actions
            if master_wrapped {
                for (idx, t) in self.tracks.iter_mut().enumerate() {
                    if Some(idx) != master_idx {
                        if t.state == TrackState::QueuedRecord {
                            t.start_recording();
                        }
                    }
                }
            }


            // Generate Metronome Click (synthesized percussion sound: woodblock style)
            let mut metro_sample = 0.0f32;
            if self.metro_enabled {
                let frames_per_beat = (self.sample_rate as f64 * 60.0) / self.metro_bpm as f64;
                self.metro_phase += 1.0;
                if self.metro_phase >= frames_per_beat {
                    self.metro_phase -= frames_per_beat;
                    self.metro_beat = (self.metro_beat + 1) % 4;
                    self.metro_is_accent = self.metro_beat == 0;
                    self.metro_click_sample = 0;
                }

                if self.metro_click_sample < self.metro_click_duration {
                    let t = self.metro_click_sample as f32 / self.sample_rate as f32;
                    // Accent beat 1 is 1600 Hz, other beats 800 Hz
                    let freq = if self.metro_is_accent { 1600.0 } else { 800.0 };
                    // Decaying exponential envelope
                    let decay = (-t * 120.0).exp();
                    let wave = (2.0 * std::f32::consts::PI * freq * t).sin();
                    metro_sample = wave * decay * self.metro_volume;
                    self.metro_click_sample += 1;
                }
            }

            // Mix metronome into audio:
            // When metro_cue_only is true, it plays alongside monitor mic (for headphones)
            // When false, it mixes directly into master output
            if self.metro_enabled {
                mix_l += metro_sample;
                mix_r += metro_sample;
            }

            // Add live mic monitoring (for headphones)
            if self.monitor_mic {
                mix_l += mic_l * self.monitor_volume;
                mix_r += mic_r * self.monitor_volume;
            }

            // Soft-limit master output to prevent harsh digital clipping
            output_frames[i] = (mix_l.clamp(-1.0, 1.0), mix_r.clamp(-1.0, 1.0));
        }

        // Publish snapshot for UI (non-blocking lock)
        if let Ok(mut snap) = self.snapshot_slot.try_lock() {
            let track_snaps: Vec<TrackSnapshot> = self.tracks.iter().map(|t| t.snapshot()).collect();
            *snap = LooperSnapshot {
                tracks: track_snaps,
                active_track_idx: self.active_track_idx,
                master_track_idx: self.master_track_idx,
                is_master_playing: self.is_master_playing,
                mic_peak_level: self.mic_peak_level.clamp(0.0, 1.0),
                monitor_mic: self.monitor_mic,
                monitor_volume: self.monitor_volume,
                input_gain: self.input_gain,
                sample_rate: self.sample_rate,
                metro_enabled: self.metro_enabled,
                metro_bpm: self.metro_bpm,
                metro_volume: self.metro_volume,
                metro_cue_only: self.metro_cue_only,
                metro_beat: self.metro_beat,
            };
        }
    }
}
