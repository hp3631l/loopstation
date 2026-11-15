pub mod audio;

use audio::{AudioCommand, AudioDevicesPayload, AudioEngine, LooperSnapshot};
use std::sync::Mutex;
use tauri::State;

pub struct AppState {
    pub engine: Mutex<Option<AudioEngine>>,
}

#[tauri::command]
fn get_looper_state(state: State<AppState>) -> Result<LooperSnapshot, String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        Ok(engine.get_snapshot())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn get_audio_devices(state: State<AppState>) -> Result<AudioDevicesPayload, String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    let cur_buf = lock.as_ref().map(|e| e.current_buffer_size).unwrap_or(256);
    Ok(audio::get_friendly_devices(cur_buf))
}

#[tauri::command]
fn set_audio_devices(
    input_name: Option<String>,
    output_name: Option<String>,
    buffer_size: Option<u32>,
    state: State<AppState>,
) -> Result<AudioDevicesPayload, String> {
    let mut lock = state.engine.lock().map_err(|e| e.to_string())?;
    let buf_sz = buffer_size.unwrap_or(256);

    // Drop previous engine streams
    *lock = None;

    // Start with selected hardware devices & requested buffer size
    let new_engine = AudioEngine::build_with_options(
        input_name.as_deref(),
        output_name.as_deref(),
        buf_sz,
    )?;

    let cur_buf = new_engine.current_buffer_size;
    *lock = Some(new_engine);

    Ok(audio::get_friendly_devices(cur_buf))
}

#[tauri::command]
fn trigger_track(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::TriggerTrack(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn select_track(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SelectTrack(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn toggle_mute(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::ToggleMute(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn toggle_solo(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::ToggleSolo(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_track_volume(track_id: usize, volume: f32, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetTrackVolume(track_id, volume));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn clear_track(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::ClearTrack(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn undo_track(track_id: usize, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::UndoTrack(track_id));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn master_play_stop(state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::MasterPlayStop);
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn clear_all(state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::ClearAll);
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_monitor_mic(enable: bool, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMonitorMic(enable));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_monitor_volume(volume: f32, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMonitorVolume(volume));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_input_gain(gain: f32, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetInputGain(gain));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_metro_enabled(enable: bool, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMetroEnabled(enable));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_metro_bpm(bpm: f32, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMetroBpm(bpm));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_metro_volume(volume: f32, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMetroVolume(volume));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[tauri::command]
fn set_metro_cue_only(cue_only: bool, state: State<AppState>) -> Result<(), String> {
    let lock = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = lock.as_ref() {
        engine.send_command(AudioCommand::SetMetroCueOnly(cue_only));
        Ok(())
    } else {
        Err("Audio engine not initialized".to_string())
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let engine = match AudioEngine::new() {
        Ok(eng) => {
            println!("AudioEngine successfully started with ultra-low-latency buffer.");
            Some(eng)
        }
        Err(e) => {
            eprintln!("Warning: Could not start audio engine: {}", e);
            None
        }
    };

    let app_state = AppState {
        engine: Mutex::new(engine),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_looper_state,
            get_audio_devices,
            set_audio_devices,
            trigger_track,
            select_track,
            toggle_mute,
            toggle_solo,
            set_track_volume,
            clear_track,
            undo_track,
            master_play_stop,
            clear_all,
            set_monitor_mic,
            set_monitor_volume,
            set_input_gain,
            set_metro_enabled,
            set_metro_bpm,
            set_metro_volume,
            set_metro_cue_only,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
