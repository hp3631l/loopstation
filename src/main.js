// RC-Beatbox Hardware Loopstation Controller

const getInvoke = () => {
  if (window.__TAURI__?.core?.invoke) {
    return window.__TAURI__.core.invoke;
  }
  if (window.__TAURI__?.invoke) {
    return window.__TAURI__.invoke;
  }
  return null;
};

const invoke = getInvoke();

const CIRCUMFERENCE = 2 * Math.PI * 70; // radius = 70

let activeTrackIdx = 0;
let monitorActive = true;
let isMasterPlaying = true;
let lastTelemetryTracks = [];

// DOM Master Elements
const tracksContainer = document.getElementById("tracks-container");
const masterTimeDisplay = document.getElementById("master-time");
const masterSyncState = document.getElementById("master-sync-state");
const masterPlayBtn = document.getElementById("master-play-btn");
const clearAllBtn = document.getElementById("clear-all-btn");
const inputGainSlider = document.getElementById("input-gain");
const gainValDisplay = document.getElementById("gain-val");
const monitorToggleBtn = document.getElementById("monitor-toggle");
const monitorVolSlider = document.getElementById("monitor-vol");
const monitorValDisplay = document.getElementById("monitor-val");
const engineStatus = document.getElementById("engine-status");
const micSelect = document.getElementById("mic-select");
const speakerSelect = document.getElementById("speaker-select");
const bufferSelect = document.getElementById("buffer-size-select");
const latencyDisplay = document.getElementById("latency-display");
const micLadder = document.getElementById("mic-ladder");

// Metronome Elements
const metroToggleBtn = document.getElementById("metro-toggle");
const metroVolSlider = document.getElementById("metro-vol");
const metroCueToggleBtn = document.getElementById("metro-cue-toggle");
const bpmValDisplay = document.getElementById("bpm-val");
const bpmUpBtn = document.getElementById("bpm-up");
const bpmDownBtn = document.getElementById("bpm-down");
const tapTempoBtn = document.getElementById("tap-tempo-btn");
const metroBeatsContainer = document.getElementById("metro-beats");

let currentBpm = 120;
let metroActive = false;
let metroCueOnly = true;
let tapTimes = [];

// Audio Device & Latency Routing
async function loadAudioDevices() {
  if (!invoke) return;
  try {
    const data = await invoke("get_audio_devices");
    populateDevices(data);
  } catch (e) {
    console.error("Failed to load audio devices:", e);
  }
}

function populateDevices(data) {
  micSelect.innerHTML = "";
  data.inputs.forEach((d) => {
    const opt = document.createElement("option");
    opt.value = d.id;
    opt.textContent = d.is_default ? `${d.display_name} (Active)` : d.display_name;
    if (d.id === data.current_input) opt.selected = true;
    micSelect.appendChild(opt);
  });

  speakerSelect.innerHTML = "";
  data.outputs.forEach((d) => {
    const opt = document.createElement("option");
    opt.value = d.id;
    opt.textContent = d.is_default ? `${d.display_name} (Active)` : d.display_name;
    if (d.id === data.current_output) opt.selected = true;
    speakerSelect.appendChild(opt);
  });

  if (data.current_buffer_size) {
    bufferSelect.value = String(data.current_buffer_size);
    const ms = ((data.current_buffer_size / 48000) * 1000).toFixed(1);
    latencyDisplay.textContent = `~${ms}ms ROUND-TRIP`;
  }
}

async function handleDeviceChange() {
  if (!invoke) return;
  try {
    const updated = await invoke("set_audio_devices", {
      inputName: micSelect.value || null,
      outputName: speakerSelect.value || null,
      bufferSize: parseInt(bufferSelect.value, 10),
    });
    populateDevices(updated);
  } catch (e) {
    console.error("Failed to update audio device:", e);
  }
}

micSelect.addEventListener("change", handleDeviceChange);
speakerSelect.addEventListener("change", handleDeviceChange);
bufferSelect.addEventListener("change", handleDeviceChange);

// Build 4 Realistic Hardware Channel Strips
function buildTrackCards() {
  tracksContainer.innerHTML = "";
  for (let i = 0; i < 4; i++) {
    const strip = document.createElement("div");
    strip.className = `channel-strip ch-${i} state-empty ${i === 0 ? "selected" : ""}`;
    strip.id = `track-card-${i}`;

    strip.innerHTML = `
      <div class="channel-header">
        <div class="channel-id-row">
          <span class="channel-num">CH 0${i + 1}</span>
          <span class="channel-key-hint">[${i + 1}]</span>
        </div>
        <span class="master-led" id="master-pill-${i}">MASTER</span>
      </div>

      <!-- Playback Mode Selector: PLAY (Loop) | MPC (One-Shot) -->
      <div class="channel-mode-row">
        <span class="mode-badge-label">MODE</span>
        <button class="channel-mode-btn mode-play" id="mode-btn-${i}" title="Toggle: PLAY (Loop) / MPC (One-Shot)" aria-label="Channel ${i + 1} Playback Mode">PLAY</button>
      </div>

      <!-- Boss RC-505 Circular Silicone Performance Pad & Radial Loop Ring -->
      <div class="pad-assembly" id="trigger-btn-${i}" role="button" tabindex="0" aria-label="Channel ${i + 1} Performance Pad">
        <div class="pad-circular-housing">
          <svg class="pad-ring-svg" viewBox="0 0 130 130" aria-hidden="true">
            <circle class="ring-track" cx="65" cy="65" r="58"></circle>
            <circle class="ring-progress" id="loop-ring-${i}" cx="65" cy="65" r="58"></circle>
          </svg>
          <div class="hardware-pad circular-silicone-pad" id="pad-body-${i}">
            <span class="pad-track-badge">CH 0${i + 1}</span>
            <span class="pad-state-text" id="state-text-${i}">EMPTY</span>
            <span class="pad-action-sub" id="trigger-label-${i}">PUNCH REC</span>
            <span class="pad-time-text" id="time-text-${i}">--</span>
          </div>
        </div>
      </div>

      <!-- Tactile Micro-Pads -->
      <div class="channel-function-row">
        <button class="func-pad" id="mute-btn-${i}" aria-label="Channel ${i + 1} Mute">MUTE</button>
        <button class="func-pad" id="solo-btn-${i}" aria-label="Channel ${i + 1} Solo">SOLO</button>
        <button class="func-pad" id="undo-btn-${i}" aria-label="Channel ${i + 1} Undo" disabled>UNDO</button>
        <button class="func-pad" id="clear-btn-${i}" aria-label="Channel ${i + 1} Clear">CLEAR</button>
      </div>

      <!-- Physical Studio Mixer Bay -->
      <div class="mixer-bay">
        <div class="fader-scale" aria-hidden="true">
          <span>+6</span>
          <span>0</span>
          <span>-6</span>
          <span>-12</span>
          <span>-24</span>
          <span>-∞</span>
        </div>

        <div class="vertical-fader-assembly">
          <div class="fader-groove"></div>
          <input
            type="range"
            class="pro-vertical-fader"
            id="vol-slider-${i}"
            min="0"
            max="1.5"
            step="0.01"
            value="1.0"
            orient="vertical"
            aria-label="Channel ${i + 1} Volume Fader"
          />
        </div>

        <div class="channel-meter-ladder" id="track-meter-${i}" aria-hidden="true">
          <div class="meter-segment seg-red"></div>
          <div class="meter-segment seg-yellow"></div>
          <div class="meter-segment seg-yellow"></div>
          <div class="meter-segment seg-green"></div>
          <div class="meter-segment seg-green"></div>
          <div class="meter-segment seg-green"></div>
          <div class="meter-segment seg-green"></div>
          <div class="meter-segment seg-green"></div>
        </div>
      </div>

      <div class="fader-footer">
        <span class="fader-db-readout" id="vol-val-${i}">0 dB</span>
      </div>
    `;

    // Click anywhere on channel strip selects it
    strip.addEventListener("click", (e) => {
      if (!e.target.closest("button") && !e.target.closest("input") && !e.target.closest(".pad-assembly")) {
        selectTrack(i);
      }
    });

    // Giant Pad Trigger (Mouse & Touch)
    const triggerBtn = strip.querySelector(`#trigger-btn-${i}`);
    
    const onPadPress = (e) => {
      selectTrack(i);
      const trackSnap = lastTelemetryTracks[i];
      const isRecorded = trackSnap && trackSnap.length_seconds > 0 && trackSnap.state !== "Empty" && trackSnap.state !== "QueuedRecord" && trackSnap.state !== "Recording";
      const mode = trackModes[i] || "Play";

      if (isRecorded && mode === "Mpc") {
        retriggerTrack(i);
      } else {
        triggerTrack(i);
      }
    };

    triggerBtn.addEventListener("pointerdown", onPadPress);
    triggerBtn.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onPadPress(e);
      }
    });

    const modeBtn = strip.querySelector(`#mode-btn-${i}`);
    modeBtn.addEventListener("click", () => {
      cycleTrackMode(i);
    });

    const muteBtn = strip.querySelector(`#mute-btn-${i}`);
    muteBtn.addEventListener("click", () => toggleMute(i));

    const soloBtn = strip.querySelector(`#solo-btn-${i}`);
    soloBtn.addEventListener("click", () => toggleSolo(i));

    const undoBtn = strip.querySelector(`#undo-btn-${i}`);
    undoBtn.addEventListener("click", () => undoTrack(i));

    const clearBtn = strip.querySelector(`#clear-btn-${i}`);
    clearBtn.addEventListener("click", () => clearTrack(i));

    const volSlider = strip.querySelector(`#vol-slider-${i}`);
    volSlider.addEventListener("input", (e) => {
      const vol = parseFloat(e.target.value);
      const db = vol === 0 ? "-∞ dB" : `${(20 * Math.log10(vol)).toFixed(1)} dB`;
      strip.querySelector(`#vol-val-${i}`).textContent = db;
      setTrackVolume(i, vol);
    });

    tracksContainer.appendChild(strip);
  }
}

// Backend Command Invokers
async function triggerTrack(idx) {
  if (invoke) {
    try {
      await invoke("trigger_track", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

async function selectTrack(idx) {
  activeTrackIdx = idx;
  if (invoke) {
    try {
      await invoke("select_track", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
  updateActiveCardHighlight();
}

async function toggleMute(idx) {
  if (invoke) {
    try {
      await invoke("toggle_mute", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

async function toggleSolo(idx) {
  if (invoke) {
    try {
      await invoke("toggle_solo", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

async function undoTrack(idx) {
  if (invoke) {
    try {
      await invoke("undo_track", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

async function clearTrack(idx) {
  if (invoke) {
    try {
      await invoke("clear_track", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

async function setTrackVolume(idx, vol) {
  if (invoke) {
    try {
      await invoke("set_track_volume", { trackId: idx, volume: vol });
    } catch (e) {
      console.error(e);
    }
  }
}

async function masterPlayStop() {
  if (invoke) {
    try {
      await invoke("master_play_stop");
    } catch (e) {
      console.error(e);
    }
  }
}

async function clearAll() {
  if (invoke) {
    try {
      await invoke("clear_all");
    } catch (e) {
      console.error(e);
    }
  }
}

// Master Controls Bindings
masterPlayBtn.addEventListener("click", masterPlayStop);
clearAllBtn.addEventListener("click", clearAll);

inputGainSlider.addEventListener("input", (e) => {
  const gain = parseFloat(e.target.value);
  gainValDisplay.textContent = `${gain.toFixed(1)}x`;
  if (invoke) {
    invoke("set_input_gain", { gain });
  }
});

monitorToggleBtn.addEventListener("click", () => {
  monitorActive = !monitorActive;
  monitorToggleBtn.classList.toggle("active", monitorActive);
  monitorToggleBtn.textContent = monitorActive ? "CUE ON" : "CUE OFF";
  if (invoke) {
    invoke("set_monitor_mic", { enable: monitorActive });
  }
});

monitorVolSlider.addEventListener("input", (e) => {
  const vol = parseFloat(e.target.value);
  monitorValDisplay.textContent = `${Math.round(vol * 100)}%`;
  if (invoke) {
    invoke("set_monitor_volume", { volume: vol });
  }
});

// Metronome Controls Bindings
function updateBpm(newBpm) {
  currentBpm = Math.max(40, Math.min(260, Math.round(newBpm)));
  bpmValDisplay.textContent = currentBpm;
  if (invoke) {
    invoke("set_metro_bpm", { bpm: currentBpm });
  }
}

bpmUpBtn.addEventListener("click", () => updateBpm(currentBpm + 1));
bpmDownBtn.addEventListener("click", () => updateBpm(currentBpm - 1));

metroToggleBtn.addEventListener("click", () => {
  metroActive = !metroActive;
  metroToggleBtn.classList.toggle("active", metroActive);
  metroToggleBtn.textContent = metroActive ? "CLICK ON" : "CLICK";
  if (invoke) {
    invoke("set_metro_enabled", { enable: metroActive });
  }
});

metroVolSlider.addEventListener("input", (e) => {
  const vol = parseFloat(e.target.value);
  if (invoke) {
    invoke("set_metro_volume", { volume: vol });
  }
});

metroCueToggleBtn.addEventListener("click", () => {
  metroCueOnly = !metroCueOnly;
  metroCueToggleBtn.classList.toggle("active", metroCueOnly);
  metroCueToggleBtn.textContent = metroCueOnly ? "CUE ONLY" : "MASTER OUT";
  if (invoke) {
    invoke("set_metro_cue_only", { cueOnly: metroCueOnly });
  }
});

tapTempoBtn.addEventListener("click", () => {
  const now = performance.now();
  tapTimes.push(now);
  if (tapTimes.length > 4) tapTimes.shift();
  if (tapTimes.length >= 2) {
    const intervals = [];
    for (let i = 1; i < tapTimes.length; i++) {
      intervals.push(tapTimes[i] - tapTimes[i - 1]);
    }
    const avgMs = intervals.reduce((a, b) => a + b, 0) / intervals.length;
    if (avgMs > 200 && avgMs < 1500) {
      const calcBpm = 60000 / avgMs;
      updateBpm(calcBpm);
    }
  }
});

function updateActiveCardHighlight() {
  for (let i = 0; i < 4; i++) {
    const strip = document.getElementById(`track-card-${i}`);
    if (strip) {
      strip.classList.toggle("selected", i === activeTrackIdx);
    }
  }
}

// Performance State & Key Bindings
let mpcKey = localStorage.getItem("loopstation_mpc_key") || "j";
let isMKeyHeld = false;
let isPKeyHeld = false;
let mKeyDownTime = 0;
let pKeyDownTime = 0;

const mpcKeyBadge = document.getElementById("mpc-key-badge");
if (mpcKeyBadge) {
  mpcKeyBadge.textContent = mpcKey.toUpperCase();
  mpcKeyBadge.title = "Click to reassign MPC Chops trigger key";
  mpcKeyBadge.style.cursor = "pointer";
  mpcKeyBadge.addEventListener("click", () => {
    const chosen = prompt("Enter a key to use for MPC Chops (e.g., J, K, B):", mpcKey.toUpperCase());
    if (chosen && chosen.trim().length > 0) {
      mpcKey = chosen.trim().toLowerCase()[0];
      localStorage.setItem("loopstation_mpc_key", mpcKey);
      mpcKeyBadge.textContent = mpcKey.toUpperCase();
    }
  });
}

async function setTrackMute(idx, muted) {
  if (invoke) {
    try {
      await invoke("set_track_mute", { trackId: idx, muted });
    } catch (e) {
      console.error(e);
    }
  }
}

async function setTrackPaused(idx, paused) {
  if (invoke) {
    try {
      await invoke("set_track_paused", { trackId: idx, paused });
    } catch (e) {
      console.error(e);
    }
  }
}

async function retriggerTrack(idx) {
  if (invoke) {
    try {
      await invoke("retrigger_track", { trackId: idx });
    } catch (e) {
      console.error(e);
    }
  }
}

// Track Playback Modes: 'Play' (Loop) | 'Mpc' (One-Shot)
const trackModes = ["Play", "Play", "Play", "Play"];

async function setTrackMode(idx, mode) {
  trackModes[idx] = mode;
  if (invoke) {
    try {
      await invoke("set_track_mode", { trackId: idx, mode });
    } catch (e) {
      console.error(e);
    }
  }
  updateModeButtonUI(idx, mode);
}

function cycleTrackMode(idx) {
  const current = trackModes[idx] || "Play";
  const next = current === "Play" ? "Mpc" : "Play";
  setTrackMode(idx, next);
}

function updateModeButtonUI(idx, mode) {
  const btn = document.getElementById(`mode-btn-${idx}`);
  if (!btn) return;
  btn.classList.remove("mode-play", "mode-mpc", "mode-total", "mode-partial");
  if (mode === "Mpc") {
    btn.textContent = "MPC";
    btn.classList.add("mode-mpc");
    btn.title = "One-Shot: Plays sample once to end and stops; strike cuts & retriggers from start";
  } else {
    btn.textContent = "PLAY";
    btn.classList.add("mode-play");
    btn.title = "Play: Continuous synchronized loop playback";
  }
}

// Keyboard shortcuts for hands-on beatbox looping
window.addEventListener("keydown", (e) => {
  if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;

  const key = e.key;

  if (key === "1") selectTrack(0);
  else if (key === "2") selectTrack(1);
  else if (key === "3") selectTrack(2);
  else if (key === "4") selectTrack(3);
  else if (key === "o" || key === "O") {
    // Cycle playback mode for active channel: PLAY <-> MPC
    cycleTrackMode(activeTrackIdx);
  } else if (key === " ") {
    e.preventDefault();
    if (e.repeat) return; // Prevent OS key-repeat stutter while held
    const trackSnap = lastTelemetryTracks[activeTrackIdx];
    const isRecorded = trackSnap && trackSnap.length_seconds > 0 && trackSnap.state !== "Empty" && trackSnap.state !== "QueuedRecord" && trackSnap.state !== "Recording";
    const mode = trackModes[activeTrackIdx] || "Play";

    if (isRecorded && mode === "Mpc") {
      retriggerTrack(activeTrackIdx);
    } else {
      triggerTrack(activeTrackIdx);
    }
  } else if (key.toLowerCase() === mpcKey.toLowerCase()) {
    // 3. MPC Behavior: Retrigger from start on strike
    e.preventDefault();
    retriggerTrack(activeTrackIdx);
  } else if (key === "m" || key === "M") {
    // 1. Mute on Hold: Mutes instantly when held down
    if (!isMKeyHeld) {
      isMKeyHeld = true;
      mKeyDownTime = performance.now();
      setTrackMute(activeTrackIdx, true);
    }
  } else if (key === "p" || key === "P") {
    // 2. Pause on Hold (Option B: tape stop/freeze)
    if (!isPKeyHeld) {
      isPKeyHeld = true;
      pKeyDownTime = performance.now();
      setTrackPaused(activeTrackIdx, true);
    }
  } else if (key === "s" || key === "S") {
    toggleSolo(activeTrackIdx);
  } else if (key === "u" || key === "U") {
    undoTrack(activeTrackIdx);
  } else if (key === "c" || key === "C") {
    if (e.shiftKey) {
      clearAll();
    } else {
      clearTrack(activeTrackIdx);
    }
  } else if (key === "t" || key === "T") {
    metroActive = !metroActive;
    metroToggleBtn.classList.toggle("active", metroActive);
    metroToggleBtn.textContent = metroActive ? "CLICK ON" : "CLICK";
    if (invoke) {
      invoke("set_metro_enabled", { enable: metroActive });
    }
  } else if (key === "Escape") {
    masterPlayStop();
  }
});

// Keyup handler to release momentary Mute and Pause
window.addEventListener("keyup", (e) => {
  if (e.target.tagName === "INPUT" || e.target.tagName === "SELECT") return;

  const key = e.key;

  if (key === "m" || key === "M") {
    if (isMKeyHeld) {
      isMKeyHeld = false;
      const duration = performance.now() - mKeyDownTime;
      if (duration < 180) {
        // Quick tap: behaves as normal persistent mute toggle
        // We already set it muted in keydown, so nothing to revert
      } else {
        // Held: release mute (unmute) upon release
        setTrackMute(activeTrackIdx, false);
      }
    }
  } else if (key === "p" || key === "P") {
    if (isPKeyHeld) {
      isPKeyHeld = false;
      setTrackPaused(activeTrackIdx, false);
    }
  }
});

// Telemetry Polling Loop (smooth 30ms updates)
async function pollLoop() {
  if (invoke) {
    try {
      const state = await invoke("get_looper_state");
      renderState(state);
    } catch (e) {
      engineStatus.classList.remove("online");
    }
  }
  requestAnimationFrame(() => setTimeout(pollLoop, 30));
}

function renderState(snap) {
  engineStatus.classList.add("online");

  activeTrackIdx = snap.active_track_idx;
  isMasterPlaying = snap.is_master_playing;
  lastTelemetryTracks = snap.tracks || [];

  // Master Stop / Play button state
  if (isMasterPlaying) {
    masterPlayBtn.classList.remove("paused");
    masterPlayBtn.querySelector(".pad-symbol").textContent = "■";
    masterPlayBtn.querySelector(".pad-label").textContent = "STOP ALL";
  } else {
    masterPlayBtn.classList.add("paused");
    masterPlayBtn.querySelector(".pad-symbol").textContent = "▶";
    masterPlayBtn.querySelector(".pad-label").textContent = "RESUME";
  }

  // Update Master Mic LED Ladder (6 dots: 3 green, 2 yellow, 1 red)
  if (micLadder) {
    const dots = micLadder.querySelectorAll(".ladder-dot");
    const peak = snap.mic_peak_level;
    dots[5].classList.toggle("on", peak > 0.05); // green 1
    dots[4].classList.toggle("on", peak > 0.20); // green 2
    dots[3].classList.toggle("on", peak > 0.40); // green 3
    dots[2].classList.toggle("on", peak > 0.65); // yellow 1
    dots[1].classList.toggle("on", peak > 0.85); // yellow 2
    dots[0].classList.toggle("on", peak > 0.95); // red clip
  }

  // Render Metronome Beat LED Indicators (4 beats)
  if (metroBeatsContainer) {
    const leds = metroBeatsContainer.querySelectorAll(".beat-led");
    leds.forEach((led, b) => {
      led.classList.toggle("on", snap.metro_enabled && snap.metro_beat === b);
    });
  }

  // Master Loop Time & Status
  let maxLen = 0;
  if (snap.master_track_idx !== null && snap.tracks[snap.master_track_idx]) {
    maxLen = snap.tracks[snap.master_track_idx].length_seconds;
    masterSyncState.textContent = `SYNC CH 0${snap.master_track_idx + 1}`;
    masterSyncState.style.color = "#f59e0b";
  } else {
    masterSyncState.textContent = "FREE-RUN";
    masterSyncState.style.color = "#64748b";
  }
  masterTimeDisplay.textContent = `${maxLen.toFixed(2)}s`;

  // Render Channel Strips
  snap.tracks.forEach((t, i) => {
    const strip = document.getElementById(`track-card-${i}`);
    if (!strip) return;

    strip.classList.toggle("selected", i === activeTrackIdx);

    // Master Indicator LED
    const masterPill = document.getElementById(`master-pill-${i}`);
    if (masterPill) {
      masterPill.style.display = snap.master_track_idx === i ? "inline-block" : "none";
    }

    // State classes on the entire strip
    const stateClassMap = {
      Empty: "state-empty",
      QueuedRecord: "state-queued",
      Recording: "state-recording",
      Playing: "state-playing",
      Overdubbing: "state-overdubbing",
      Muted: "state-muted",
    };

    Object.values(stateClassMap).forEach((cls) => strip.classList.remove(cls));
    strip.classList.add(stateClassMap[t.state] || "state-empty");

    // Center state text
    const stateText = document.getElementById(`state-text-${i}`);
    if (stateText) {
      const stateLabels = {
        Empty: "EMPTY",
        QueuedRecord: "WAIT",
        Recording: "RECORD",
        Playing: "PLAY",
        Overdubbing: "OVERDUB",
        Muted: "MUTED",
      };
      stateText.textContent = stateLabels[t.state] || "EMPTY";
    }

    // Action sub label
    const triggerLabel = document.getElementById(`trigger-label-${i}`);
    if (triggerLabel) {
      const btnLabels = {
        Empty: "PUNCH REC",
        QueuedRecord: "SYNCING...",
        Recording: "PUNCH OUT",
        Playing: "OVERDUB",
        Overdubbing: "PLAY",
        Muted: "UNMUTE",
      };
      triggerLabel.textContent = btnLabels[t.state] || "REC";
    }

    // Center time text
    const timeText = document.getElementById(`time-text-${i}`);
    if (timeText) {
      timeText.textContent = t.length_seconds > 0 ? `${t.length_seconds.toFixed(1)}s` : "--";
    }

    // Circular radial loop ring (r = 58, circ = 2 * PI * 58 ≈ 364.4)
    const ring = document.getElementById(`loop-ring-${i}`);
    if (ring) {
      const circ = 2 * Math.PI * 58;
      const offset = circ * (1 - t.playhead_norm);
      ring.style.strokeDasharray = `${circ}`;
      ring.style.strokeDashoffset = `${offset}`;
    }

    // Channel 8-Segment Vertical LED Meter
    const meter = document.getElementById(`track-meter-${i}`);
    if (meter) {
      const segs = meter.querySelectorAll(".meter-segment");
      const peak = t.peak_level;
      // Index 7 is bottom green, index 0 is top red
      segs[7].classList.toggle("lit", peak > 0.05);
      segs[6].classList.toggle("lit", peak > 0.15);
      segs[5].classList.toggle("lit", peak > 0.30);
      segs[4].classList.toggle("lit", peak > 0.45);
      segs[3].classList.toggle("lit", peak > 0.60);
      segs[2].classList.toggle("lit", peak > 0.75);
      segs[1].classList.toggle("lit", peak > 0.88);
      segs[0].classList.toggle("lit", peak > 0.96);
    }

    // Mute button
    const muteBtn = document.getElementById(`mute-btn-${i}`);
    if (muteBtn) {
      muteBtn.classList.toggle("active-mute", t.is_muted);
    }

    // Solo button
    const soloBtn = document.getElementById(`solo-btn-${i}`);
    if (soloBtn) {
      soloBtn.classList.toggle("active-solo", t.is_solo);
    }

    // Undo button
    const undoBtn = document.getElementById(`undo-btn-${i}`);
    if (undoBtn) {
      undoBtn.disabled = !t.has_undo;
      undoBtn.classList.toggle("active-undo", t.has_undo);
    }

    // Sync mode button UI if telemetry provides it
    if (t.mode && trackModes[i] !== t.mode) {
      trackModes[i] = t.mode;
      updateModeButtonUI(i, t.mode);
    }
  });
}

// In-Place Update Checker & Self-Updater
const CURRENT_VERSION = "v0.1.3";
const GITHUB_REPO = "hp3631l/loopstation";
const checkUpdateBtn = document.getElementById("check-update-btn");
const versionTag = document.getElementById("current-version-tag");

if (versionTag) {
  versionTag.textContent = CURRENT_VERSION;
}

let latestDownloadUrl = null;
let isUpdating = false;

async function checkForUpdates(manual = false) {
  if (!checkUpdateBtn || isUpdating) return;
  checkUpdateBtn.textContent = "CHECKING...";
  checkUpdateBtn.classList.remove("available", "up-to-date");

  try {
    const res = await fetch(`https://api.github.com/repos/${GITHUB_REPO}/releases/latest`);
    if (!res.ok) throw new Error("Network error");
    const release = await res.json();
    const latestTag = release.tag_name || "";

    // Find the Linux AppImage asset
    if (release.assets && Array.isArray(release.assets)) {
      const appImageAsset = release.assets.find(
        (a) => a.name && a.name.endsWith(".AppImage")
      );
      if (appImageAsset) {
        latestDownloadUrl = appImageAsset.browser_download_url;
      }
    }

    if (latestTag && latestTag !== CURRENT_VERSION && latestDownloadUrl) {
      checkUpdateBtn.textContent = `UPDATE (${latestTag})`;
      checkUpdateBtn.classList.add("available");
      checkUpdateBtn.title = `Click to automatically install ${latestTag} in-place`;
    } else {
      checkUpdateBtn.textContent = "UP TO DATE";
      checkUpdateBtn.classList.add("up-to-date");
      if (manual) {
        setTimeout(() => {
          checkUpdateBtn.textContent = "CHECK UPDATE";
          checkUpdateBtn.classList.remove("up-to-date");
        }, 3000);
      }
    }
  } catch (err) {
    console.error("Update check failed:", err);
    checkUpdateBtn.textContent = manual ? "ERR CHECKING" : "CHECK UPDATE";
  }
}

if (checkUpdateBtn) {
  checkUpdateBtn.addEventListener("click", async () => {
    if (isUpdating) return;

    if (checkUpdateBtn.classList.contains("available") && latestDownloadUrl) {
      if (!invoke) return;
      isUpdating = true;
      checkUpdateBtn.textContent = "DOWNLOADING...";
      checkUpdateBtn.classList.add("available");

      try {
        await invoke("apply_in_place_update", { downloadUrl: latestDownloadUrl });
      } catch (e) {
        console.error("Update failed:", e);
        checkUpdateBtn.textContent = "ERR UPDATING";
        isUpdating = false;
        setTimeout(() => checkForUpdates(true), 4000);
      }
    } else {
      checkForUpdates(true);
    }
  });
}

// Start
buildTrackCards();
updateActiveCardHighlight();
loadAudioDevices();
pollLoop();
checkForUpdates(false);
