// ── Entry point ─────────────────────────────────────────────────────

import init, { GameBoy } from "./pkg/gb_emu.js";
import { createRenderer } from "./renderer.js";
import { createPanels } from "./panels.js";
import { createInput } from "./input.js";
import { createCamera } from "./camera.js";
import { createSave } from "./save.js";

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
};

async function main() {
  const wasm = await init();
  state.wasmMemory = wasm.memory;

  // DOM
  const $ = (id) => document.getElementById(id);
  const frameInfo = $("frame-info");
  const cameraPanel = $("panel-camera");

  // Modules
  const renderer = createRenderer(state, $("screen"), $("tile-canvas"));
  const { updateAll, buttonState } = createPanels(state, {
    disPre: $("disasm-pre"),
    memPre: $("mem-pre"),
    serialPre: $("serial-output"),
  });
  const input = createInput(state, buttonState);
  const camera = createCamera(state, {
    cameraStatus: $("camera-status"),
    liveCapture: $("live-capture"),
    webcamPreview: $("webcam-preview"),
    webcamStatus: $("webcam-status"),
    btnWebcamToggle: $("btn-webcam-toggle"),
    galleryGrid: $("gallery-grid"),
  });
  const save = createSave(state);

  // Load ROM helper
  async function loadRom(data, name) {
    // Stop camera before loading new ROM (clean up intervals, etc.)
    camera.stopCamera();

    // Create fresh emulator instance
    state.emulator = new GameBoy();
    state.emulator.load_rom(data);

    $("rom-name").textContent = name;
    state.currentRomName = name.replace(/\.(gb|gbc|bin|rom)$/i, "");

    // Reset emulator state
    state.paused = true;
    state.frameCounter = 0;
    state.panelUpdateCounter = 0;

    const cartType = data.length >= 0x148 ? data[0x147] : 0;
    state.isGameBoyCamera = cartType === 0xfc;

    if (state.isGameBoyCamera) {
      cameraPanel.style.display = "block";
      camera.startCamera();
      await camera.initWebcam();
      if (camera.isWebcamEnabled()) camera.captureFrame();
    } else {
      cameraPanel.style.display = "none";
    }

    renderer.renderScreen();
    updateAll();
    renderer.renderTiles();
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
      renderer.renderTiles();
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

  // Save/Load
  $("download-save").onclick = () => save.download();
  $("load-save").addEventListener("change", async (e) => {
    if (e.target.files[0]) await save.load(e.target.files[0]);
    e.target.value = "";
  });

  // Keyboard
  input.attach();

  // Frame loop
  function tick() {
    if (state.emulator) {
      if (!state.paused) {
        if (
          state.isGameBoyCamera &&
          camera.isWebcamEnabled() &&
          state.frameCounter % 4 === 0
        ) {
          camera.captureFrame();
        }
        for (let i = 0; i < state.speed; i++) state.emulator.step_frame();
        state.frameCounter += state.speed;
        renderer.renderScreen();
        if (state.isGameBoyCamera) camera.updateLiveView();

        if (++state.panelUpdateCounter >= 10) {
          state.panelUpdateCounter = 0;
          updateAll();
          renderer.renderTiles();
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
        renderer.renderTiles();
      } else if (state.stepMode === "instruction") {
        state.emulator.step_instruction();
        state.stepMode = null;
        renderer.renderScreen();
        updateAll();
        renderer.renderTiles();
      }

      frameInfo.textContent = `Frame ${state.frameCounter} ${state.paused ? "[PAUSED]" : ""}`;
    }
    requestAnimationFrame(tick);
  }

  requestAnimationFrame(tick);
}

main().catch(console.error);
