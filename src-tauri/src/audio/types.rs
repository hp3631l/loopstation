use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrackState {
    Empty,
    QueuedRecord,
    Recording,
    Playing,
    Overdubbing,
    Muted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackSnapshot {
    pub id: usize,
    pub state: TrackState,
    pub playhead_norm: f32, // 0.0 to 1.0 progress through loop
    pub length_seconds: f32,
    pub volume: f32,
    pub is_muted: bool,
    pub is_solo: bool,
    pub peak_level: f32,    // 0.0 to 1.0 for UI audio meter
    pub has_undo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LooperSnapshot {
    pub tracks: Vec<TrackSnapshot>,
    pub active_track_idx: usize,
    pub master_track_idx: Option<usize>,
    pub is_master_playing: bool,
    pub mic_peak_level: f32,
    pub monitor_mic: bool,
    pub monitor_volume: f32,
    pub input_gain: f32,
    pub sample_rate: u32,
}

pub use super::devices::{AudioDeviceInfo, AudioDevicesPayload};

#[derive(Debug)]
pub enum AudioCommand {
    SelectTrack(usize),
    TriggerTrack(usize), // Tap record/play/overdub (Boss RC-505 style)
    ToggleMute(usize),
    ToggleSolo(usize),
    SetTrackVolume(usize, f32),
    ClearTrack(usize),
    UndoTrack(usize),
    MasterPlayStop,
    ClearAll,
    SetMonitorMic(bool),
    SetMonitorVolume(f32),
    SetInputGain(f32),
}
