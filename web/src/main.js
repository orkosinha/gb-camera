// ── Entry point ─────────────────────────────────────────────────────

import init, { GameBoy } from "./pkg/gb_emu.js";
import { createRenderer } from "./renderer.js";
import { createPanels } from "./panels.js";
import { createInput } from "./input.js";
import { createCamera } from "./camera.js";
import { createSave } from "./save.js";
import { createMotion } from "./motion.js";

const state = {
  emulator: null,
  wasmMemory: null,
  paused: true,
  stepMode: null,
  speed: 1,
  frameCounter: 0,
  panelUpdateCounter: 0,
  memViewAddr: 0x0000,
  currentRomName: "game",
  isGameBoyCamera: false,
  isMbc7: false,
  tileViewBank: 0,
};

async function main() {
  const wasm = await init();
  state.wasmMemory = wasm.memory;

  // DOM
  const $ = (id) => document.getElementById(id);
  const frameInfo  = $("frame-info");
  const cameraPanel = $("panel-camera");
  const motionPanel = $("panel-motion");

  // Modules
  const renderer = createRenderer(state, $("screen"), $("tile-canvas"), $("bg-map-canvas"));
  const { updateAll, buttonState } = createPanels(state, {
    disPre: $("disasm-pre"),
    memPre: $("mem-pre"),
    serialPre: $("serial-output"),
  });
  const input = createInput(state, buttonState);
  const camera = createCamera(state, {
    cameraStatus: $("camera-status"),
    cameraSettings: $("camera-settings"),
    liveCapture: $("live-capture"),
    webcamPreview: $("webcam-preview"),
    webcamStatus: $("webcam-status"),
    btnWebcamToggle: $("btn-webcam-toggle"),
    galleryGrid: $("gallery-grid"),
  });
  const save = createSave(state);

  // MBC7 accelerometer input
  const tiltDot = $("tilt-dot");
  const motion = createMotion((x, y) => {
    if (state.emulator && state.isMbc7) state.emulator.set_accelerometer(x, y);
    // Update visual indicator: map ±MAX (0x3000) to ±35px from centre
    const px = 50 + (x / 0x3000) * 35;
    const py = 50 + (y / 0x3000) * 35;
    tiltDot.style.left = `${px.toFixed(1)}%`;
    tiltDot.style.top  = `${py.toFixed(1)}%`;
  });

  // CGB tile viewer bank selector
  function updateBankButtons() {
    $("btn-tile-bank0").classList.toggle("active-bank", state.tileViewBank === 0);
    $("btn-tile-bank1").classList.toggle("active-bank", state.tileViewBank === 1);
  }
  $("btn-tile-bank0").onclick = () => {
    state.tileViewBank = 0;
    updateBankButtons();
    if (state.emulator) renderer.renderTiles(0);
  };
  $("btn-tile-bank1").onclick = () => {
    state.tileViewBank = 1;
    updateBankButtons();
    if (state.emulator) renderer.renderTiles(1);
  };

  // Load ROM helper
  async function loadRom(data, name) {
    // Stop camera before loading new ROM (clean up intervals, etc.)
    camera.stopCamera();

    // Create fresh emulator instance
    state.emulator = new GameBoy();
    state.emulator.load_rom(data, name.endsWith(".gbc"));

    $("rom-name").textContent = name;
    state.currentRomName = name.replace(/\.(gb|gbc|bin|rom)$/i, "");

    // Reset emulator state
    state.paused = true;
    state.frameCounter = 0;
    state.panelUpdateCounter = 0;

    const cartType = data.length >= 0x148 ? data[0x147] : 0;
    state.isGameBoyCamera = cartType === 0xfc;
    state.isMbc7          = cartType === 0x22;

    // Stop any active motion tracking when loading a new ROM
    motion.disable();

    if (state.isGameBoyCamera) {
      cameraPanel.style.display = "block";
      camera.startCamera();
      await camera.initWebcam();
      if (camera.isWebcamEnabled()) camera.captureFrame();
    } else {
      cameraPanel.style.display = "none";
    }

    motionPanel.style.display = state.isMbc7 ? "block" : "none";
    if (state.isMbc7) {
      // Auto-enable accelerometer on MBC7 detection
      const ok = await motion.enableMotion();
      if (ok) {
        $("motion-status").textContent = motion.isMouse() ? "Mouse drag active" : "Accelerometer active";
      } else {
        motion.enableMouse($("screen"));
        $("motion-status").textContent = "Mouse drag active";
      }
      $("btn-motion-toggle").textContent = "Disable Accelerometer";
    }

    renderer.renderScreen();
    updateAll();
    // Show bank selector only in CGB mode; reset to bank 0
    state.tileViewBank = 0;
    $("tile-bank-select").style.display = state.emulator.is_cgb_mode() ? "flex" : "none";
    updateBankButtons();
    renderer.renderTiles(state.tileViewBank);
    renderer.renderBgMap();
  }

  // Load default ROM (film.gb)
  try {
    const resp = await fetch("./pkg/film.gb");
    if (resp.ok) {
      const data = new Uint8Array(await resp.arrayBuffer());
      await loadRom(data, "film.gb");
      state.paused = false; // Auto-play the default ROM
    }
  } catch (e) {
    console.log("Default ROM not available:", e);
  }

  // ROM loading from file picker
  $("rom-file").addEventListener("change", async (e) => {
    const file = e.target.files[0];
    if (!file) return;

    const data = new Uint8Array(await file.arrayBuffer());
    await loadRom(data, file.name);
  });

  // GB Buttons
  $("btn-play").onclick = () => {
    state.paused = false;
  };
  $("btn-pause").onclick = () => {
    state.paused = true;
    if (state.emulator) {
      updateAll();
      renderer.renderTiles(state.tileViewBank);
      renderer.renderBgMap();
    }
  };
  $("btn-step-frame").onclick = () => {
    if (!state.emulator) return;
    state.paused = true;
    state.stepMode = "frame";
  };
  $("btn-step-instr").onclick = () => {
    if (!state.emulator) return;
    state.paused = true;
    state.stepMode = "instruction";
  };
  $("speed-select").onchange = (e) => {
    state.speed = parseFloat(e.target.value);
  };

  // Memory navigation
  const memInput = $("mem-addr-input");
  const memGo = () => {
    const val = parseInt(memInput.value, 16);
    if (!isNaN(val)) {
      state.memViewAddr = val & 0xffff;
      memInput.value = state.memViewAddr
        .toString(16)
        .toUpperCase()
        .padStart(4, "0");
      if (state.emulator) updateAll();
    }
  };
  const memJump = (delta) => {
    state.memViewAddr = (state.memViewAddr + delta) & 0xffff;
    memInput.value = state.memViewAddr
      .toString(16)
      .toUpperCase()
      .padStart(4, "0");
    if (state.emulator) updateAll();
  };

  $("mem-goto").onclick = memGo;
  memInput.onkeydown = (e) => {
    if (e.key === "Enter") memGo();
  };
  $("mem-prev-page").onclick = () => memJump(-0x100);
  $("mem-prev").onclick = () => memJump(-0x10);
  $("mem-next").onclick = () => memJump(0x10);
  $("mem-next-page").onclick = () => memJump(0x100);

  // Camera
  $("btn-webcam-toggle").onclick = () => {
    camera.isWebcamEnabled() ? camera.stopWebcam() : camera.initWebcam();
  };

  // MBC7 accelerometer
  $("btn-motion-toggle").onclick = async () => {
    if (motion.isEnabled()) {
      motion.disable();
      $("motion-status").textContent = "Accelerometer disabled";
      $("btn-motion-toggle").textContent = "Enable Accelerometer";
    } else {
      // Try real motion first; fall back to mouse drag on desktop
      const ok = await motion.enableMotion();
      if (ok) {
        $("motion-status").textContent = motion.isMouse()
          ? "Mouse drag active" : "Motion active";
      } else {
        motion.enableMouse($("screen"));
        $("motion-status").textContent = "Mouse drag active";
      }
      $("btn-motion-toggle").textContent = "Disable Accelerometer";
    }
  };

  // Save/Load
  $("download-save").onclick = () => save.download();
  $("load-save").addEventListener("change", async (e) => {
    if (e.target.files[0]) await save.load(e.target.files[0]);
    e.target.value = "";
  });

  // Keyboard
  input.attach();

  // Frame loop
  let cameraTickCounter = 0;
  function tick() {
    if (state.emulator) {
      if (!state.paused) {
        // Capture webcam every 4 ticks (independent of speed)
        if (
          state.isGameBoyCamera &&
          camera.isWebcamEnabled() &&
          ++cameraTickCounter >= 4
        ) {
          cameraTickCounter = 0;
          camera.captureFrame();
        }
        for (let i = 0; i < state.speed; i++) state.emulator.step_frame();
        state.frameCounter += state.speed;
        renderer.renderScreen();
        if (state.isGameBoyCamera) camera.updateLiveView();

        if (++state.panelUpdateCounter >= 10) {
          state.panelUpdateCounter = 0;
          updateAll();
          renderer.renderTiles(state.tileViewBank);
          renderer.renderBgMap();
        }
      } else if (state.stepMode === "frame") {
        if (state.isGameBoyCamera && camera.isWebcamEnabled())
          camera.captureFrame();
        state.emulator.step_frame();
        state.frameCounter++;
        state.stepMode = null;
        renderer.renderScreen();
        if (state.isGameBoyCamera) camera.updateLiveView();
        updateAll();
        renderer.renderTiles(state.tileViewBank);
        renderer.renderBgMap();
      } else if (state.stepMode === "instruction") {
        state.emulator.step_instruction();
        state.stepMode = null;
        renderer.renderScreen();
        updateAll();
        renderer.renderTiles(state.tileViewBank);
        renderer.renderBgMap();
      }

      frameInfo.textContent = `Frame ${state.frameCounter} ${state.paused ? "[PAUSED]" : ""}`;
    }
    requestAnimationFrame(tick);
  }

  requestAnimationFrame(tick);
}

main().catch(console.error);
