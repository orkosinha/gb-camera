// ── Entry point — WASM init, frame loop, event wiring ───────────────

import init, { GameBoy } from '../pkg/gb_emu.js';
import { createRenderer } from './renderer.js';
import { createPanels } from './panels.js';
import { createInput } from './input.js';
import { createCamera } from './camera.js';
import { createSave } from './save.js';

// ── State ───────────────────────────────────────────────────────────

const state = {
    emulator: null,
    wasmMemory: null,
    paused: true,
    stepMode: null,       // null | 'frame' | 'instruction'
    speed: 1,
    frameCounter: 0,
    panelUpdateCounter: 0,
    memViewAddr: 0x0000,
    currentRomName: 'game',
    isGameBoyCamera: false,
};

// ── Boot ────────────────────────────────────────────────────────────

async function main() {
    const wasm = await init();
    state.wasmMemory = wasm.memory;

    // DOM refs
    const screenCanvas = document.getElementById('screen');
    const tileCanvas = document.getElementById('tile-canvas');
    const frameInfo = document.getElementById('frame-info');

    const panelRefs = {
        cpuPre: document.getElementById('cpu-pre'),
        ppuPre: document.getElementById('ppu-pre'),
        disPre: document.getElementById('disasm-pre'),
        memPre: document.getElementById('mem-pre'),
        timerPre: document.getElementById('timer-pre'),
        intPre: document.getElementById('int-pre'),
        serialPre: document.getElementById('serial-output'),
    };

    const cameraRefs = {
        cameraStatus: document.getElementById('camera-status'),
        liveCapture: document.getElementById('live-capture'),
        webcamPreview: document.getElementById('webcam-preview'),
        webcamStatus: document.getElementById('webcam-status'),
        btnWebcamToggle: document.getElementById('btn-webcam-toggle'),
        galleryGrid: document.getElementById('gallery-grid'),
    };

    // Module factories
    const renderer = createRenderer(state, screenCanvas, tileCanvas);
    const { updateAll, buttonState } = createPanels(state, panelRefs);
    const input = createInput(state, buttonState);
    const camera = createCamera(state, cameraRefs);
    const save = createSave(state);

    // ── ROM loading ─────────────────────────────────────────────────

    document.getElementById('rom-file').addEventListener('change', async (e) => {
        const file = e.target.files[0];
        if (!file) return;
        const data = new Uint8Array(await file.arrayBuffer());

        state.emulator = new GameBoy();
        state.emulator.load_rom(data);
        document.getElementById('rom-name').textContent = file.name;
        state.currentRomName = file.name.replace(/\.(gb|gbc|bin|rom)$/i, '');

        // Detect Game Boy Camera (cart type 0xFC)
        const cartType = data.length >= 0x148 ? data[0x147] : 0;
        state.isGameBoyCamera = (cartType === 0xFC);

        if (state.isGameBoyCamera) {
            camera.startCamera();
            await camera.initWebcam();
            if (camera.isWebcamEnabled()) camera.captureFrame();
        } else {
            camera.stopCamera();
        }

        state.paused = true;
        state.frameCounter = 0;
        renderer.renderScreen();
        updateAll();
        renderer.renderTiles();
    });

    // ── Control buttons ─────────────────────────────────────────────

    document.getElementById('btn-play').addEventListener('click', (e) => {
        state.paused = false;
        e.target.blur();
    });
    document.getElementById('btn-pause').addEventListener('click', (e) => {
        state.paused = true;
        if (state.emulator) { updateAll(); renderer.renderTiles(); }
        e.target.blur();
    });
    document.getElementById('btn-step-frame').addEventListener('click', (e) => {
        if (!state.emulator) return;
        if (!state.paused) state.paused = true;
        state.stepMode = 'frame';
        e.target.blur();
    });
    document.getElementById('btn-step-instr').addEventListener('click', (e) => {
        if (!state.emulator) return;
        if (!state.paused) state.paused = true;
        state.stepMode = 'instruction';
        e.target.blur();
    });

    document.getElementById('speed-select').addEventListener('change', (e) => {
        state.speed = parseFloat(e.target.value);
    });

    // ── Memory address input ────────────────────────────────────────

    document.getElementById('mem-goto').addEventListener('click', () => {
        const val = parseInt(document.getElementById('mem-addr-input').value, 16);
        if (!isNaN(val)) { state.memViewAddr = val & 0xFFFF; if (state.emulator) updateAll(); }
    });
    document.getElementById('mem-addr-input').addEventListener('keydown', (e) => {
        if (e.key === 'Enter') {
            document.getElementById('mem-goto').click();
            e.preventDefault();
        }
    });

    // ── Webcam toggle ───────────────────────────────────────────────

    document.getElementById('btn-webcam-toggle').addEventListener('click', () => {
        if (camera.isWebcamEnabled()) {
            camera.stopWebcam();
        } else {
            camera.initWebcam();
        }
    });

    // ── Save / Load ─────────────────────────────────────────────────

    document.getElementById('download-save').addEventListener('click', () => save.download());
    document.getElementById('load-save').addEventListener('change', async (e) => {
        const file = e.target.files[0];
        if (file) await save.load(file);
        e.target.value = '';
    });

    // ── Keyboard ────────────────────────────────────────────────────

    input.attach();

    // ── Frame loop ──────────────────────────────────────────────────

    function runFrame() {
        if (!state.emulator) {
            requestAnimationFrame(runFrame);
            return;
        }

        if (!state.paused) {
            // Capture webcam every 4 frames (~15fps)
            if (state.isGameBoyCamera && camera.isWebcamEnabled() && state.frameCounter % 4 === 0) {
                camera.captureFrame();
            }

            for (let i = 0; i < state.speed; i++) {
                state.emulator.step_frame();
            }
            state.frameCounter += state.speed;
            renderer.renderScreen();

            if (state.isGameBoyCamera) {
                camera.updateLiveView();
            }

            state.panelUpdateCounter++;
            if (state.panelUpdateCounter >= 10) {
                state.panelUpdateCounter = 0;
                updateAll();
                renderer.renderTiles();
            }
        } else if (state.stepMode === 'frame') {
            if (state.isGameBoyCamera && camera.isWebcamEnabled()) {
                camera.captureFrame();
            }
            state.emulator.step_frame();
            state.frameCounter++;
            state.stepMode = null;
            renderer.renderScreen();
            if (state.isGameBoyCamera) camera.updateLiveView();
            updateAll();
            renderer.renderTiles();
        } else if (state.stepMode === 'instruction') {
            state.emulator.step_instruction();
            state.stepMode = null;
            renderer.renderScreen();
            updateAll();
            renderer.renderTiles();
        }

        frameInfo.textContent = `Frame ${String(state.frameCounter).padStart(7)}  ${state.paused ? 'PAUSED ' : 'RUNNING'}`;
        requestAnimationFrame(runFrame);
    }

    requestAnimationFrame(runFrame);
}

main().catch(console.error);
