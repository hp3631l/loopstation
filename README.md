# Beatbox Loopstation

A real-time, hardware-style audio loopstation designed for live beatboxing, built with Rust and Tauri.

## Features
- **4 Dedicated Loop Tracks** with sample-accurate loop boundary alignment.
- **Master Loop Synchronization**: The first recorded track automatically defines the loop length and tempo cycle. Subsequent tracks automatically quantize and sync to the master cycle.
- **Click-Free Seam Micro-Crossfading**: Eliminates audio pops and discontinuities at loop endpoints.
- **Non-Destructive Overdubbing**: Layer multiple vocal lines onto any track with full 1-step undo.
- **Tactile Performance Interface**:
  - Rotating visual loop dials.
  - Multi-color LED states (Empty, Record, Play, Overdub, Queued, Muted).
  - Dynamic audio peak meters.
  - Live headphone monitoring toggle with volume slider and mic input gain control.
- **Keyboard Shortcuts for Live Beatboxing**:
  - `[1]`, `[2]`, `[3]`, `[4]`: Select Active Track
  - `[SPACE]`: Trigger Record / Play / Overdub on active track
  - `[M]`: Mute / Unmute active track
  - `[S]`: Solo active track
  - `[U]`: Undo last overdub on active track
  - `[C]`: Clear active track (`Shift + C`: Clear all tracks)
  - `[ESC]`: Master Stop / Resume all playback

## Running the App on Linux
Run directly from terminal:
```bash
/home/hariprakash/Projects/loopstation/src-tauri/target/debug/tauri-app
```
Or run the portable AppImage:
```bash
/home/hariprakash/Projects/loopstation/src-tauri/target/debug/bundle/appimage/Beatbox\ Loopstation_0.1.0_amd64.AppImage
```

## Running / Building on Windows
1. Clone or copy this repository to your Windows machine.
2. Ensure [Node.js](https://nodejs.org) and [Rust](https://rustup.rs) are installed.
3. In PowerShell or Command Prompt, run:
```powershell
npm install
npm run tauri build
```
This produces a standalone Windows `.exe` and `.msi` installer in `src-tauri/target/release/bundle/msi/`.
