# RC-Beatbox Loopstation

A native, ultra-low-latency 4-channel audio loopstation built for live beatboxers. Inspired by hardware loopstations (Boss RC-505 Mk2) with physical tactile silicone pads, master loop synchronization, and seamless overdubbing.

---

## ⚡ Download & Run (Plug & Play)

Pre-built binaries are available on the [Releases Page](https://github.com/hp3631l/loopstation/releases/latest).

### Linux (AppImage)
1. Download **`loopstation-x86_64.AppImage`**.
2. Make it executable and run:
   ```bash
   chmod +x loopstation-x86_64.AppImage
   ./loopstation-x86_64.AppImage
   ```

### Windows (Standalone Zip)
1. Download **`loopstation-windows.zip`**.
2. Extract the zip and double-click **`tauri-app.exe`** (or run the installer).
3. Plug in your mic and headphones—the app automatically detects your audio hardware via WASAPI.

---

## 🎹 Live Performance Shortcuts

Designed for hands-on live beatboxing without touching the mouse:

| Key | Action | Description |
| :--- | :--- | :--- |
| **`[1]` - `[4]`** | Select Track | Focus channel strip 1, 2, 3, or 4 |
| **`[SPACE]`** | Punch In / Out | Record ➔ Play ➔ Overdub on active track |
| **`[M]`** | Mute | Toggle mute on active track |
| **`[S]`** | Solo | Solo active track (mutes other channels) |
| **`[U]`** | Undo | Undo last overdub layer |
| **`[C]`** | Clear Track | Erase loop on active track |
| **`[Shift + C]`**| Clear All | Instant wipe of all 4 channels |
| **`[ESC]`** | Master Stop | Stop or resume all playback simultaneously |

---

## 🛠️ Build From Source

### Prerequisites
- [Node.js](https://nodejs.org) (v18+)
- [Rust](https://rustup.rs) (1.75+)

### Clone & Build
```bash
git clone https://github.com/hp3631l/loopstation.git
cd loopstation
npm install
npm run tauri build
```
Binaries will be placed in `src-tauri/target/release/bundle/`.
