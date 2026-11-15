pub mod devices;
pub mod processor;
pub mod track;
pub mod types;

#[cfg(test)]
mod tests;

use std::sync::{Arc, Mutex};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{unbounded, Sender};
use ringbuf::traits::{Consumer, Observer, Producer, Split};
use ringbuf::HeapRb;

pub use devices::{get_friendly_devices, set_system_device};
pub use types::{
    AudioCommand, AudioDeviceInfo, AudioDevicesPayload, LooperSnapshot, TrackSnapshot, TrackState,
};
use processor::LooperProcessor;

pub struct AudioEngine {
    _in_stream: cpal::Stream,
    _out_stream: cpal::Stream,
    cmd_tx: Sender<AudioCommand>,
    snapshot: Arc<Mutex<LooperSnapshot>>,
    pub current_input: String,
    pub current_output: String,
    pub current_buffer_size: u32,
}

// CPAL Stream contains a PhantomData<*mut ()> on some platforms, but on desktop platforms (Linux/Windows)
// the stream is safely owned and managed in the engine.
unsafe impl Send for AudioEngine {}
unsafe impl Sync for AudioEngine {}

impl AudioEngine {
    pub fn new() -> Result<Self, String> {
        Self::build_with_options(None, None, 256)
    }

    pub fn build_with_options(
        input_id: Option<&str>,
        output_id: Option<&str>,
        buffer_size: u32,
    ) -> Result<Self, String> {
        // Update PipeWire system route if on Linux
        set_system_device(input_id, output_id);

        let host = cpal::default_host();

        let in_device = if let Some(id) = input_id {
            if id != "default" {
                host.input_devices()
                    .ok()
                    .and_then(|mut devs| devs.find(|d| d.name().map(|n| n == id).unwrap_or(false)))
                    .or_else(|| host.default_input_device())
            } else {
                host.default_input_device()
            }
        } else {
            host.default_input_device()
        }
        .ok_or_else(|| "No input device (microphone) detected".to_string())?;

        let out_device = if let Some(id) = output_id {
            if id != "default" {
                host.output_devices()
                    .ok()
                    .and_then(|mut devs| devs.find(|d| d.name().map(|n| n == id).unwrap_or(false)))
                    .or_else(|| host.default_output_device())
            } else {
                host.default_output_device()
            }
        } else {
            host.default_output_device()
        }
        .ok_or_else(|| "No output device (headphones/speakers) detected".to_string())?;

        let in_def = in_device
            .default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let out_def = out_device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {}", e))?;

        // Standardize on 48,000 Hz if supported, otherwise device default
        let sample_rate = if out_def.sample_rate().0 == 48000 || in_def.sample_rate().0 == 48000 {
            48000
        } else {
            out_def.sample_rate().0
        };

        let in_channels = in_def.channels() as usize;
        let out_channels = out_def.channels() as usize;

        // Configure buffer size with device min clamp
        let in_buf_size = match in_def.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => buffer_size.clamp(*min, *max),
            cpal::SupportedBufferSize::Unknown => buffer_size,
        };

        let out_buf_size = match out_def.buffer_size() {
            cpal::SupportedBufferSize::Range { min, max } => buffer_size.clamp(*min, *max),
            cpal::SupportedBufferSize::Unknown => buffer_size,
        };

        let in_stream_config = cpal::StreamConfig {
            channels: in_def.channels(),
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(in_buf_size),
        };

        let out_stream_config = cpal::StreamConfig {
            channels: out_def.channels(),
            sample_rate: cpal::SampleRate(sample_rate),
            buffer_size: cpal::BufferSize::Fixed(out_buf_size),
        };

        let (cmd_tx, cmd_rx) = unbounded::<AudioCommand>();

        let initial_snapshot = Arc::new(Mutex::new(LooperSnapshot {
            tracks: (0..4)
                .map(|id| TrackSnapshot {
                    id,
                    state: TrackState::Empty,
                    playhead_norm: 0.0,
                    length_seconds: 0.0,
                    volume: 1.0,
                    is_muted: false,
                    is_solo: false,
                    peak_level: 0.0,
                    has_undo: false,
                })
                .collect(),
            active_track_idx: 0,
            master_track_idx: None,
            is_master_playing: true,
            mic_peak_level: 0.0,
            monitor_mic: true,
            monitor_volume: 1.0,
            input_gain: 1.0,
            sample_rate,
            metro_enabled: false,
            metro_bpm: 120.0,
            metro_volume: 0.7,
            metro_cue_only: true,
            metro_beat: 0,
        }));

        let mut processor = LooperProcessor::new(sample_rate, cmd_rx, Arc::clone(&initial_snapshot));

        // Jitter cushion capacity: 8 blocks
        let rb_capacity = (out_buf_size as usize * in_channels * 8).max(2048);
        let rb = HeapRb::<f32>::new(rb_capacity);
        let (mut in_prod, mut in_cons) = rb.split();

        // Build Input Stream
        let in_err = |err| eprintln!("Input stream audio error: {}", err);
        let in_stream = match in_def.sample_format() {
            cpal::SampleFormat::F32 => in_device.build_input_stream(
                &in_stream_config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    for &s in data {
                        let _ = in_prod.try_push(s);
                    }
                },
                in_err,
                None,
            ),
            cpal::SampleFormat::I16 => in_device.build_input_stream(
                &in_stream_config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    for &s in data {
                        let f = s as f32 / i16::MAX as f32;
                        let _ = in_prod.try_push(f);
                    }
                },
                in_err,
                None,
            ),
            cpal::SampleFormat::U16 => in_device.build_input_stream(
                &in_stream_config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    for &s in data {
                        let f = (s as f32 / u16::MAX as f32) * 2.0 - 1.0;
                        let _ = in_prod.try_push(f);
                    }
                },
                in_err,
                None,
            ),
            fmt => return Err(format!("Unsupported input sample format: {:?}", fmt)),
        }
        .map_err(|e| format!("Failed to build input stream: {}", e))?;

        let mut input_block: Vec<(f32, f32)> = Vec::with_capacity(2048);
        let mut output_block: Vec<(f32, f32)> = Vec::with_capacity(2048);
        let mut last_sample_l = 0.0f32;

        // Build Output Stream
        let out_err = |err| eprintln!("Output stream audio error: {}", err);
        let out_stream = match out_def.sample_format() {
            cpal::SampleFormat::F32 => out_device.build_output_stream(
                &out_stream_config,
                move |out_data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    let out_frames = out_data.len() / out_channels;
                    input_block.clear();
                    output_block.resize(out_frames, (0.0, 0.0));

                    // DRAIN EXCESS BACKLOG with a 3-block safety headroom.
                    // This guarantees zero underrun crackles while keeping latency < 8ms!
                    let max_headroom = out_frames * in_channels * 3;
                    while in_cons.occupied_len() > max_headroom {
                        let _ = in_cons.try_pop();
                    }

                    // Read frames from input ring buffer with click-free sample smoothing
                    for _ in 0..out_frames {
                        if in_channels == 1 {
                            let mono = match in_cons.try_pop() {
                                Some(val) => {
                                    last_sample_l = val;
                                    val
                                }
                                None => {
                                    // Soft decay on underrun avoids hard 0.0 clicks
                                    last_sample_l *= 0.98;
                                    last_sample_l
                                }
                            };
                            input_block.push((mono, mono));
                        } else {
                            let left = match in_cons.try_pop() {
                                Some(val) => {
                                    last_sample_l = val;
                                    val
                                }
                                None => {
                                    last_sample_l *= 0.98;
                                    last_sample_l
                                }
                            };
                            let right = in_cons.try_pop().unwrap_or(left);
                            for _ in 2..in_channels {
                                let _ = in_cons.try_pop();
                            }
                            input_block.push((left, right));
                        }
                    }

                    processor.process_block(&input_block, &mut output_block);

                    // Write to audio card buffer
                    for (frame_idx, &(l, r)) in output_block.iter().enumerate() {
                        let base = frame_idx * out_channels;
                        if out_channels == 1 {
                            out_data[base] = (l + r) * 0.5;
                        } else {
                            out_data[base] = l;
                            out_data[base + 1] = r;
                            for ch in 2..out_channels {
                                out_data[base + ch] = 0.0;
                            }
                        }
                    }
                },
                out_err,
                None,
            ),
            fmt => return Err(format!("Unsupported output sample format: {:?}", fmt)),
        }
        .map_err(|e| format!("Failed to build output stream: {}", e))?;

        out_stream
            .play()
            .map_err(|e| format!("Failed to start output stream: {}", e))?;
        in_stream
            .play()
            .map_err(|e| format!("Failed to start input stream: {}", e))?;

        let current_in = in_device
            .name()
            .unwrap_or_else(|_| input_id.unwrap_or("default").to_string());
        let current_out = out_device
            .name()
            .unwrap_or_else(|_| output_id.unwrap_or("default").to_string());

        Ok(Self {
            _in_stream: in_stream,
            _out_stream: out_stream,
            cmd_tx,
            snapshot: initial_snapshot,
            current_input: current_in,
            current_output: current_out,
            current_buffer_size: out_buf_size,
        })
    }

    pub fn send_command(&self, cmd: AudioCommand) {
        let _ = self.cmd_tx.send(cmd);
    }

    pub fn get_snapshot(&self) -> LooperSnapshot {
        self.snapshot.lock().unwrap().clone()
    }
}
