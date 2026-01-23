import init, { GameBoy } from './pkg/gb_emu.js';

let emulator = null;
let wasmMemory = null;
let canvas = null;
let ctx = null;
let imageData = null;
let animationId = null;
let lastFrameTime = 0;
let running = false;
let frameCounter = 0;
let debugEnabled = true; // Enable debug logging

// Webcam-related variables
let webcamStream = null;
let webcamVideo = null;
let webcamCanvas = null;
let webcamCtx = null;
let webcamEnabled = false;
let isGameBoyCamera = false;

// Current ROM name for save file naming
let currentRomName = 'game';

const FRAME_DURATION = 1000 / 59.73; // ~16.74ms per frame
const SCALE = 3;

// Game Boy Camera image dimensions
const CAMERA_WIDTH = 128;
const CAMERA_HEIGHT = 112;

// Debug logging helper
function debugLog(msg) {
    if (debugEnabled) {
        console.log(`[JS] ${msg}`);
    }
}

// Button constants (must match Rust)
const BUTTON_A = 0;
const BUTTON_B = 1;
const BUTTON_SELECT = 2;
const BUTTON_START = 3;
const BUTTON_RIGHT = 4;
const BUTTON_LEFT = 5;
const BUTTON_UP = 6;
const BUTTON_DOWN = 7;

// Initialize webcam for Game Boy Camera
async function initWebcam() {
    if (webcamEnabled) return true;

    try {
        debugLog('Requesting webcam access...');

        // Create hidden video element for webcam
        webcamVideo = document.createElement('video');
        webcamVideo.setAttribute('autoplay', '');
        webcamVideo.setAttribute('playsinline', '');

        // Create canvas for processing webcam frames
        webcamCanvas = document.createElement('canvas');
        webcamCanvas.width = CAMERA_WIDTH;
        webcamCanvas.height = CAMERA_HEIGHT;
        webcamCtx = webcamCanvas.getContext('2d', { willReadFrequently: true });

        // Request webcam access
        webcamStream = await navigator.mediaDevices.getUserMedia({
            video: {
                width: { ideal: 640 },
                height: { ideal: 480 },
                facingMode: 'user'
            },
            audio: false
        });

        webcamVideo.srcObject = webcamStream;
        await webcamVideo.play();

        webcamEnabled = true;
        debugLog(`Webcam initialized: ${webcamVideo.videoWidth}x${webcamVideo.videoHeight}`);
        return true;
    } catch (err) {
        console.error('Failed to access webcam:', err);
        debugLog(`Webcam error: ${err.message}`);
        return false;
    }
}

// Capture a frame from webcam and send to emulator
function captureWebcamFrame() {
    if (!webcamEnabled || !webcamVideo || !emulator) return;

    // Draw video frame to canvas, scaled and cropped to 128x112
    const videoAspect = webcamVideo.videoWidth / webcamVideo.videoHeight;
    const targetAspect = CAMERA_WIDTH / CAMERA_HEIGHT;

    let srcX = 0, srcY = 0, srcW = webcamVideo.videoWidth, srcH = webcamVideo.videoHeight;

    // Crop to match aspect ratio
    if (videoAspect > targetAspect) {
        // Video is wider - crop sides
        srcW = webcamVideo.videoHeight * targetAspect;
        srcX = (webcamVideo.videoWidth - srcW) / 2;
    } else {
        // Video is taller - crop top/bottom
        srcH = webcamVideo.videoWidth / targetAspect;
        srcY = (webcamVideo.videoHeight - srcH) / 2;
    }

    // Draw scaled and cropped frame (mirror horizontally for selfie view)
    webcamCtx.save();
    webcamCtx.scale(-1, 1);
    webcamCtx.drawImage(
        webcamVideo,
        srcX, srcY, srcW, srcH,
        -CAMERA_WIDTH, 0, CAMERA_WIDTH, CAMERA_HEIGHT
    );
    webcamCtx.restore();

    // Get image data and convert to grayscale
    const imageData = webcamCtx.getImageData(0, 0, CAMERA_WIDTH, CAMERA_HEIGHT);
    const grayscale = new Uint8Array(CAMERA_WIDTH * CAMERA_HEIGHT);

    for (let i = 0; i < grayscale.length; i++) {
        const r = imageData.data[i * 4];
        const g = imageData.data[i * 4 + 1];
        const b = imageData.data[i * 4 + 2];
        // Convert to grayscale using luminance formula
        grayscale[i] = Math.round(0.299 * r + 0.587 * g + 0.114 * b);
    }

    // Send to emulator
    emulator.set_camera_image(grayscale);
}

// Stop webcam
function stopWebcam() {
    if (webcamStream) {
        webcamStream.getTracks().forEach(track => track.stop());
        webcamStream = null;
    }
    webcamEnabled = false;
    debugLog('Webcam stopped');
}

// Download save file (cartridge RAM)
function downloadSave() {
    if (!emulator) {
        alert('No ROM loaded');
        return;
    }

    const saveData = emulator.get_cartridge_ram();
    const blob = new Blob([saveData], { type: 'application/octet-stream' });
    const url = URL.createObjectURL(blob);

    const a = document.createElement('a');
    a.href = url;
    a.download = `${currentRomName}.sav`;
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    debugLog(`Save file downloaded: ${currentRomName}.sav (${saveData.length} bytes)`);
}

// Load save file (cartridge RAM)
async function loadSave(file) {
    if (!emulator) {
        alert('Load a ROM first before loading a save file');
        return;
    }

    const buffer = await file.arrayBuffer();
    const saveData = new Uint8Array(buffer);

    emulator.load_cartridge_ram(saveData);
    debugLog(`Save file loaded: ${file.name} (${saveData.length} bytes)`);
}

// Setup save file controls
function setupSaveControls() {
    const downloadBtn = document.getElementById('download-save');
    const loadInput = document.getElementById('load-save');

    if (downloadBtn) {
        downloadBtn.addEventListener('click', downloadSave);
    }

    if (loadInput) {
        loadInput.addEventListener('change', async (e) => {
            const file = e.target.files[0];
            if (file) {
                await loadSave(file);
            }
            // Reset input so the same file can be loaded again
            e.target.value = '';
        });
    }
}

async function initialize() {
    const wasm = await init();
    wasmMemory = wasm.memory;

    canvas = document.getElementById('screen');
    canvas.width = 160;
    canvas.height = 144;
    canvas.style.width = `${160 * SCALE}px`;
    canvas.style.height = `${144 * SCALE}px`;

    ctx = canvas.getContext('2d');
    imageData = ctx.createImageData(160, 144);

    // Fill with Game Boy green
    for (let i = 0; i < imageData.data.length; i += 4) {
        imageData.data[i] = 155;     // R
        imageData.data[i + 1] = 188; // G
        imageData.data[i + 2] = 15;  // B
        imageData.data[i + 3] = 255; // A
    }
    ctx.putImageData(imageData, 0, 0);

    setupInputHandlers();
    setupFileInput();
    setupSaveControls();

    console.log('Emulator initialized');
}

function setupFileInput() {
    const input = document.getElementById('rom-input');
    input.addEventListener('change', async (e) => {
        const file = e.target.files[0];
        if (!file) return;

        debugLog(`Loading ROM file: ${file.name}`);
        stopEmulation();

        const buffer = await file.arrayBuffer();
        const romData = new Uint8Array(buffer);

        debugLog(`ROM file read: ${romData.length} bytes`);

        // Log first few bytes of ROM for debugging
        const headerBytes = Array.from(romData.slice(0x100, 0x110))
            .map(b => b.toString(16).padStart(2, '0'))
            .join(' ');
        debugLog(`ROM entry point bytes (0x100-0x10F): ${headerBytes}`);

        // Log ROM header info
        const title = String.fromCharCode(...romData.slice(0x134, 0x144)).replace(/\0/g, '');
        const cartType = romData[0x147];
        debugLog(`ROM title: "${title}", cart type: 0x${cartType.toString(16).padStart(2, '0')}`);

        try {
            debugLog('Creating GameBoy instance...');
            emulator = new GameBoy();

            debugLog('Loading ROM into emulator...');
            emulator.load_rom(romData);

            debugLog(`ROM loaded successfully: ${file.name} (${romData.length} bytes)`);

            // Set ROM name for save files (remove extension)
            currentRomName = file.name.replace(/\.(gb|gbc|bin)$/i, '');

            // Show save controls
            const saveControls = document.getElementById('save-controls');
            if (saveControls) {
                saveControls.style.display = 'block';
            }

            // Check if this is Game Boy Camera (cart type 0xFC)
            isGameBoyCamera = (cartType === 0xFC);
            if (isGameBoyCamera) {
                debugLog('Game Boy Camera ROM detected - initializing webcam...');
                const webcamReady = await initWebcam();
                if (webcamReady) {
                    debugLog('Webcam ready for Game Boy Camera');
                    // Capture initial frame
                    captureWebcamFrame();
                } else {
                    debugLog('WARNING: Webcam not available - camera captures will be blank');
                }
            }

            // Log initial VRAM info
            debugLog('Calling log_vram_info...');
            emulator.log_vram_info();

            frameCounter = 0;
            startEmulation();
        } catch (err) {
            console.error('Failed to load ROM:', err);
            debugLog(`ERROR: Failed to load ROM: ${err}`);
            alert(`Failed to load ROM: ${err}`);
        }
    });
}

function setupInputHandlers() {
    const keyMap = {
        'ArrowRight': BUTTON_RIGHT,
        'ArrowLeft': BUTTON_LEFT,
        'ArrowUp': BUTTON_UP,
        'ArrowDown': BUTTON_DOWN,
        'KeyZ': BUTTON_A,
        'KeyX': BUTTON_B,
        'Enter': BUTTON_START,
        'ShiftLeft': BUTTON_SELECT,
        'ShiftRight': BUTTON_SELECT,
    };

    document.addEventListener('keydown', (e) => {
        if (!emulator || !running) return;

        const button = keyMap[e.code];
        if (button !== undefined) {
            e.preventDefault();
            emulator.set_button(button, true);
        }
    });

    document.addEventListener('keyup', (e) => {
        if (!emulator) return;

        const button = keyMap[e.code];
        if (button !== undefined) {
            e.preventDefault();
            emulator.set_button(button, false);
        }
    });
}

function startEmulation() {
    if (running) return;
    running = true;
    lastFrameTime = performance.now();
    animationId = requestAnimationFrame(runFrame);
    console.log('Emulation started');
}

function stopEmulation() {
    running = false;
    if (animationId) {
        cancelAnimationFrame(animationId);
        animationId = null;
    }
}

function runFrame(timestamp) {
    if (!running) return;

    const elapsed = timestamp - lastFrameTime;

    if (elapsed >= FRAME_DURATION) {
        lastFrameTime = timestamp - (elapsed % FRAME_DURATION);

        // Capture webcam frame for Game Boy Camera (every 4 frames = ~15fps)
        if (isGameBoyCamera && webcamEnabled && frameCounter % 4 === 0) {
            captureWebcamFrame();
        }

        // Run one frame
        emulator.step_frame();
        frameCounter++;

        // Render
        const ptr = emulator.frame_buffer_ptr();
        const len = emulator.frame_buffer_len();
        const frameBuffer = new Uint8Array(wasmMemory.buffer, ptr, len);

        // Debug: Check frame buffer content on first few frames
        if (frameCounter <= 5 || frameCounter % 300 === 0) {
            // Count non-white pixels (0xFF is white)
            let nonWhiteCount = 0;
            let uniqueColors = new Set();
            for (let i = 0; i < frameBuffer.length; i += 4) {
                const r = frameBuffer[i];
                uniqueColors.add(r);
                if (r !== 0xFF) {
                    nonWhiteCount++;
                }
            }
            debugLog(`Frame ${frameCounter}: non-white pixels=${nonWhiteCount}, unique colors=${Array.from(uniqueColors).map(c => '0x' + c.toString(16)).join(',')}`);

            // Log the emulator's internal debug info
            emulator.get_debug_info();
        }

        imageData.data.set(frameBuffer);
        ctx.putImageData(imageData, 0, 0);
    }

    animationId = requestAnimationFrame(runFrame);
}

initialize().catch(console.error);
