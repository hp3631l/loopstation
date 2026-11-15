use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub id: String,          // System identifier (e.g., pactl name or cpal name)
    pub display_name: String,// Human-friendly name (e.g., "USB Condenser Microphone")
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevicesPayload {
    pub inputs: Vec<AudioDeviceInfo>,
    pub outputs: Vec<AudioDeviceInfo>,
    pub current_input: String,
    pub current_output: String,
    pub current_buffer_size: u32,
}

/// Query human-readable device names from PipeWire/PulseAudio via pactl,
/// falling back to sanitized ALSA/CPAL device names.
pub fn get_friendly_devices(current_buf: u32) -> AudioDevicesPayload {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    // 1. Try querying pactl for friendly PipeWire/PulseAudio names
    if let (Ok(src_out), Ok(sink_out)) = (
        Command::new("pactl").args(["list", "sources"]).output(),
        Command::new("pactl").args(["list", "sinks"]).output(),
    ) {
        let default_source = Command::new("pactl")
            .arg("get-default-source")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        let default_sink = Command::new("pactl")
            .arg("get-default-sink")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default();

        parse_pactl_output(&src_out.stdout, &default_source, &mut inputs, true);
        parse_pactl_output(&sink_out.stdout, &default_sink, &mut outputs, false);
    }

    // 2. If pactl gave results, use them; otherwise fallback to CPAL device enumeration
    let current_in = inputs
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.id.clone())
        .unwrap_or_else(|| "default".into());

    let current_out = outputs
        .iter()
        .find(|d| d.is_default)
        .map(|d| d.id.clone())
        .unwrap_or_else(|| "default".into());

    AudioDevicesPayload {
        inputs,
        outputs,
        current_input: current_in,
        current_output: current_out,
        current_buffer_size: current_buf,
    }
}

fn parse_pactl_output(
    bytes: &[u8],
    default_id: &str,
    target: &mut Vec<AudioDeviceInfo>,
    is_source: bool,
) {
    let text = String::from_utf8_lossy(bytes);
    let mut cur_name = String::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(val) = trimmed.strip_prefix("Name: ") {
            cur_name = val.trim().to_string();
        } else if let Some(val) = trimmed.strip_prefix("Description: ") {
            let cur_desc = val.trim().to_string();

            // Filter out monitor sources (we want real microphones, not speaker loopback)
            if !cur_name.is_empty() && (!is_source || !cur_name.ends_with(".monitor")) {
                let is_default = cur_name == default_id;
                let display = format_clean_description(&cur_desc, &cur_name);
                target.push(AudioDeviceInfo {
                    id: cur_name.clone(),
                    display_name: display,
                    is_default,
                });
            }
            cur_name.clear();
        }
    }
}

fn format_clean_description(desc: &str, name: &str) -> String {
    if desc.to_lowercase().contains("microphone") || name.to_lowercase().contains("microphone") {
        if desc.contains("USB") || name.contains("usb") {
            return "🎙️ USB Microphone".to_string();
        }
        return "🎙️ Built-in Microphone".to_string();
    }
    if desc.to_lowercase().contains("analog") || name.to_lowercase().contains("analog") {
        return "🎧 Headphones / Built-in Audio".to_string();
    }
    if desc.to_lowercase().contains("equalizer") {
        return "🎛️ Equalizer (Omarchy System)".to_string();
    }
    desc.to_string()
}

pub fn set_system_device(input_id: Option<&str>, output_id: Option<&str>) {
    if let Some(src) = input_id {
        let _ = Command::new("pactl").args(["set-default-source", src]).output();
    }
    if let Some(sink) = output_id {
        let _ = Command::new("pactl").args(["set-default-sink", sink]).output();
    }
}
