use serde::{Deserialize, Serialize};
use cpal::traits::{DeviceTrait, HostTrait};

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

/// Query human-readable device names cross-platform:
/// - On Linux: reads PipeWire/PulseAudio via pactl with fallback to cpal
/// - On Windows / other OS: queries native Windows WASAPI/MME devices directly via cpal
pub fn get_friendly_devices(current_buf: u32) -> AudioDevicesPayload {
    let mut inputs = Vec::new();
    let mut outputs = Vec::new();

    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
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
    }

    // Cross-platform fallback / Windows native WASAPI enumeration
    if inputs.is_empty() || outputs.is_empty() {
        let host = cpal::default_host();
        let default_in_name = host.default_input_device().and_then(|d| d.name().ok());
        let default_out_name = host.default_output_device().and_then(|d| d.name().ok());

        if inputs.is_empty() {
            if let Ok(devs) = host.input_devices() {
                for dev in devs {
                    if let Ok(name) = dev.name() {
                        let is_default = default_in_name.as_ref().map(|d| d == &name).unwrap_or(false);
                        let display = format_clean_description(&name, &name);
                        inputs.push(AudioDeviceInfo {
                            id: name.clone(),
                            display_name: display,
                            is_default,
                        });
                    }
                }
            }
        }

        if outputs.is_empty() {
            if let Ok(devs) = host.output_devices() {
                for dev in devs {
                    if let Ok(name) = dev.name() {
                        let is_default = default_out_name.as_ref().map(|d| d == &name).unwrap_or(false);
                        let display = format_clean_description(&name, &name);
                        outputs.push(AudioDeviceInfo {
                            id: name.clone(),
                            display_name: display,
                            is_default,
                        });
                    }
                }
            }
        }
    }

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

#[cfg(target_os = "linux")]
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
    let lower_desc = desc.to_lowercase();
    let lower_name = name.to_lowercase();
    if lower_desc.contains("microphone") || lower_name.contains("microphone") || lower_desc.contains("mic") {
        if desc.starts_with("🎙️") {
            return desc.to_string();
        }
        return format!("🎙️ {}", desc);
    }
    if lower_desc.contains("headphone") || lower_desc.contains("speakers") || lower_desc.contains("analog") || lower_desc.contains("output") {
        if desc.starts_with("🎧") {
            return desc.to_string();
        }
        return format!("🎧 {}", desc);
    }
    if lower_desc.contains("equalizer") {
        return "🎛️ Equalizer (System)".to_string();
    }
    desc.to_string()
}

pub fn set_system_device(input_id: Option<&str>, output_id: Option<&str>) {
    #[cfg(target_os = "linux")]
    {
        use std::process::Command;
        if let Some(src) = input_id {
            let _ = Command::new("pactl").args(["set-default-source", src]).output();
        }
        if let Some(sink) = output_id {
            let _ = Command::new("pactl").args(["set-default-sink", sink]).output();
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (input_id, output_id);
    }
}
