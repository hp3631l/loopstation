use crossbeam_channel::Receiver;
use std::sync::{Arc, Mutex};

use super::track::LoopTrack;
use super::types::{AudioCommand, LooperSnapshot, TrackSnapshot, TrackState};

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
                // Play -> Overdub
                track.start_overdub();
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
            };
        }
    }
}
