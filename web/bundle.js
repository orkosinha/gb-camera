// pkg/gb_emu.js
var GameBoy = class {
  __destroy_into_raw() {
    const ptr = this.__wbg_ptr;
    this.__wbg_ptr = 0;
    GameBoyFinalization.unregister(this);
    return ptr;
  }
  free() {
    const ptr = this.__destroy_into_raw();
    wasm.__wbg_gameboy_free(ptr, 0);
  }
  /**
   * Clear the serial output buffer.
   */
  clear_serial_output() {
    wasm.gameboy_clear_serial_output(this.__wbg_ptr);
  }
  /**
   * @returns {number}
   */
  frame_buffer_len() {
    const ret = wasm.gameboy_frame_buffer_len(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * @returns {number}
   */
  frame_buffer_ptr() {
    const ret = wasm.gameboy_frame_buffer_ptr(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * @returns {Uint8Array}
   */
  get_cartridge_ram() {
    const ret = wasm.gameboy_get_cartridge_ram(this.__wbg_ptr);
    var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
    wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
    return v1;
  }
  /**
   * Get debug info about the emulator state and log to console.
   * @returns {string}
   */
  get_debug_info() {
    let deferred1_0;
    let deferred1_1;
    try {
      const ret = wasm.gameboy_get_debug_info(this.__wbg_ptr);
      deferred1_0 = ret[0];
      deferred1_1 = ret[1];
      return getStringFromWasm0(ret[0], ret[1]);
    } finally {
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
  }
  /**
   * Get frame count for debugging.
   * @returns {number}
   */
  get_frame_count() {
    const ret = wasm.gameboy_get_frame_count(this.__wbg_ptr);
    return ret >>> 0;
  }
  /**
   * Get total instruction count for debugging.
   * @returns {bigint}
   */
  get_instruction_count() {
    const ret = wasm.gameboy_get_instruction_count(this.__wbg_ptr);
    return BigInt.asUintN(64, ret);
  }
  /**
   * Get serial output as a string (for test ROM debugging).
   * @returns {string}
   */
  get_serial_output() {
    let deferred1_0;
    let deferred1_1;
    try {
      const ret = wasm.gameboy_get_serial_output(this.__wbg_ptr);
      deferred1_0 = ret[0];
      deferred1_1 = ret[1];
      return getStringFromWasm0(ret[0], ret[1]);
    } finally {
      wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
    }
  }
  /**
   * Check if camera image is ready for capture.
   * @returns {boolean}
   */
  is_camera_ready() {
    const ret = wasm.gameboy_is_camera_ready(this.__wbg_ptr);
    return ret !== 0;
  }
  /**
   * @param {Uint8Array} data
   */
  load_cartridge_ram(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    wasm.gameboy_load_cartridge_ram(this.__wbg_ptr, ptr0, len0);
  }
  /**
   * @param {Uint8Array} rom_data
   */
  load_rom(rom_data) {
    const ptr0 = passArray8ToWasm0(rom_data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    const ret = wasm.gameboy_load_rom(this.__wbg_ptr, ptr0, len0);
    if (ret[1]) {
      throw takeFromExternrefTable0(ret[0]);
    }
  }
  /**
   * Log a message to the browser console.
   * @param {string} msg
   */
  log(msg) {
    const ptr0 = passStringToWasm0(msg, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
    const len0 = WASM_VECTOR_LEN;
    wasm.gameboy_log(this.__wbg_ptr, ptr0, len0);
  }
  /**
   * Log detailed VRAM tile data for debugging.
   */
  log_vram_info() {
    wasm.gameboy_log_vram_info(this.__wbg_ptr);
  }
  constructor() {
    const ret = wasm.gameboy_new();
    this.__wbg_ptr = ret >>> 0;
    GameBoyFinalization.register(this, this.__wbg_ptr, this);
    return this;
  }
  /**
   * @param {number} button
   * @param {boolean} pressed
   */
  set_button(button, pressed) {
    wasm.gameboy_set_button(this.__wbg_ptr, button, pressed);
  }
  /**
   * Set camera image data from webcam.
   * Expects 128x112 pixels as raw 8-bit grayscale (0=black, 255=white).
   * Sensor emulation will process this during capture with exposure, gain, and dithering.
   * @param {Uint8Array} data
   */
  set_camera_image(data) {
    const ptr0 = passArray8ToWasm0(data, wasm.__wbindgen_malloc);
    const len0 = WASM_VECTOR_LEN;
    wasm.gameboy_set_camera_image(this.__wbg_ptr, ptr0, len0);
  }
  step_frame() {
    wasm.gameboy_step_frame(this.__wbg_ptr);
  }
};
if (Symbol.dispose) GameBoy.prototype[Symbol.dispose] = GameBoy.prototype.free;
function __wbg_get_imports() {
  const import0 = {
    __proto__: null,
    __wbg___wbindgen_throw_be289d5034ed271b: function(arg0, arg1) {
      throw new Error(getStringFromWasm0(arg0, arg1));
    },
    __wbg_error_7534b8e9a36f1ab4: function(arg0, arg1) {
      let deferred0_0;
      let deferred0_1;
      try {
        deferred0_0 = arg0;
        deferred0_1 = arg1;
        console.error(getStringFromWasm0(arg0, arg1));
      } finally {
        wasm.__wbindgen_free(deferred0_0, deferred0_1, 1);
      }
    },
    __wbg_log_6b5ca2e6124b2808: function(arg0) {
      console.log(arg0);
    },
    __wbg_new_8a6f238a6ece86ea: function() {
      const ret = new Error();
      return ret;
    },
    __wbg_stack_0ed75d68575b0f3c: function(arg0, arg1) {
      const ret = arg1.stack;
      const ptr1 = passStringToWasm0(ret, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
      const len1 = WASM_VECTOR_LEN;
      getDataViewMemory0().setInt32(arg0 + 4 * 1, len1, true);
      getDataViewMemory0().setInt32(arg0 + 4 * 0, ptr1, true);
    },
    __wbg_warn_f7ae1b2e66ccb930: function(arg0) {
      console.warn(arg0);
    },
    __wbindgen_cast_0000000000000001: function(arg0, arg1) {
      const ret = getStringFromWasm0(arg0, arg1);
      return ret;
    },
    __wbindgen_init_externref_table: function() {
      const table = wasm.__wbindgen_externrefs;
      const offset = table.grow(4);
      table.set(0, void 0);
      table.set(offset + 0, void 0);
      table.set(offset + 1, null);
      table.set(offset + 2, true);
      table.set(offset + 3, false);
    }
  };
  return {
    __proto__: null,
    "./gb_emu_bg.js": import0
  };
}
var GameBoyFinalization = typeof FinalizationRegistry === "undefined" ? { register: () => {
}, unregister: () => {
} } : new FinalizationRegistry((ptr) => wasm.__wbg_gameboy_free(ptr >>> 0, 1));
function getArrayU8FromWasm0(ptr, len) {
  ptr = ptr >>> 0;
  return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}
var cachedDataViewMemory0 = null;
function getDataViewMemory0() {
  if (cachedDataViewMemory0 === null || cachedDataViewMemory0.buffer.detached === true || cachedDataViewMemory0.buffer.detached === void 0 && cachedDataViewMemory0.buffer !== wasm.memory.buffer) {
    cachedDataViewMemory0 = new DataView(wasm.memory.buffer);
  }
  return cachedDataViewMemory0;
}
function getStringFromWasm0(ptr, len) {
  ptr = ptr >>> 0;
  return decodeText(ptr, len);
}
var cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
  if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
    cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
  }
  return cachedUint8ArrayMemory0;
}
function passArray8ToWasm0(arg, malloc) {
  const ptr = malloc(arg.length * 1, 1) >>> 0;
  getUint8ArrayMemory0().set(arg, ptr / 1);
  WASM_VECTOR_LEN = arg.length;
  return ptr;
}
function passStringToWasm0(arg, malloc, realloc) {
  if (realloc === void 0) {
    const buf = cachedTextEncoder.encode(arg);
    const ptr2 = malloc(buf.length, 1) >>> 0;
    getUint8ArrayMemory0().subarray(ptr2, ptr2 + buf.length).set(buf);
    WASM_VECTOR_LEN = buf.length;
    return ptr2;
  }
  let len = arg.length;
  let ptr = malloc(len, 1) >>> 0;
  const mem = getUint8ArrayMemory0();
  let offset = 0;
  for (; offset < len; offset++) {
    const code = arg.charCodeAt(offset);
    if (code > 127) break;
    mem[ptr + offset] = code;
  }
  if (offset !== len) {
    if (offset !== 0) {
      arg = arg.slice(offset);
    }
    ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
    const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
    const ret = cachedTextEncoder.encodeInto(arg, view);
    offset += ret.written;
    ptr = realloc(ptr, len, offset, 1) >>> 0;
  }
  WASM_VECTOR_LEN = offset;
  return ptr;
}
function takeFromExternrefTable0(idx) {
  const value = wasm.__wbindgen_externrefs.get(idx);
  wasm.__externref_table_dealloc(idx);
  return value;
}
var cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
var MAX_SAFARI_DECODE_BYTES = 2146435072;
var numBytesDecoded = 0;
function decodeText(ptr, len) {
  numBytesDecoded += len;
  if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
    cachedTextDecoder = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
    cachedTextDecoder.decode();
    numBytesDecoded = len;
  }
  return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}
var cachedTextEncoder = new TextEncoder();
if (!("encodeInto" in cachedTextEncoder)) {
  cachedTextEncoder.encodeInto = function(arg, view) {
    const buf = cachedTextEncoder.encode(arg);
    view.set(buf);
    return {
      read: arg.length,
      written: buf.length
    };
  };
}
var WASM_VECTOR_LEN = 0;
var wasmModule;
var wasm;
function __wbg_finalize_init(instance, module) {
  wasm = instance.exports;
  wasmModule = module;
  cachedDataViewMemory0 = null;
  cachedUint8ArrayMemory0 = null;
  wasm.__wbindgen_start();
  return wasm;
}
async function __wbg_load(module, imports) {
  if (typeof Response === "function" && module instanceof Response) {
    if (typeof WebAssembly.instantiateStreaming === "function") {
      try {
        return await WebAssembly.instantiateStreaming(module, imports);
      } catch (e) {
        const validResponse = module.ok && expectedResponseType(module.type);
        if (validResponse && module.headers.get("Content-Type") !== "application/wasm") {
          console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);
        } else {
          throw e;
        }
      }
    }
    const bytes = await module.arrayBuffer();
    return await WebAssembly.instantiate(bytes, imports);
  } else {
    const instance = await WebAssembly.instantiate(module, imports);
    if (instance instanceof WebAssembly.Instance) {
      return { instance, module };
    } else {
      return instance;
    }
  }
  function expectedResponseType(type) {
    switch (type) {
      case "basic":
      case "cors":
      case "default":
        return true;
    }
    return false;
  }
}
async function __wbg_init(module_or_path) {
  if (wasm !== void 0) return wasm;
  if (module_or_path !== void 0) {
    if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
      ({ module_or_path } = module_or_path);
    } else {
      console.warn("using deprecated parameters for the initialization function; pass a single object instead");
    }
  }
  if (module_or_path === void 0) {
    module_or_path = new URL("gb_emu_bg.wasm", import.meta.url);
  }
  const imports = __wbg_get_imports();
  if (typeof module_or_path === "string" || typeof Request === "function" && module_or_path instanceof Request || typeof URL === "function" && module_or_path instanceof URL) {
    module_or_path = fetch(module_or_path);
  }
  const { instance, module } = await __wbg_load(await module_or_path, imports);
  return __wbg_finalize_init(instance, module);
}

// index.js
var emulator = null;
var wasmMemory = null;
var canvas = null;
var ctx = null;
var imageData = null;
var animationId = null;
var lastFrameTime = 0;
var running = false;
var frameCounter = 0;
var debugEnabled = true;
var webcamStream = null;
var webcamVideo = null;
var webcamCanvas = null;
var webcamCtx = null;
var webcamEnabled = false;
var isGameBoyCamera = false;
var currentRomName = "game";
var FRAME_DURATION = 1e3 / 59.73;
var SCALE = 3;
var CAMERA_WIDTH = 128;
var CAMERA_HEIGHT = 112;
function debugLog(msg) {
  if (debugEnabled) {
    console.log(`[JS] ${msg}`);
  }
}
var BUTTON_A = 0;
var BUTTON_B = 1;
var BUTTON_SELECT = 2;
var BUTTON_START = 3;
var BUTTON_RIGHT = 4;
var BUTTON_LEFT = 5;
var BUTTON_UP = 6;
var BUTTON_DOWN = 7;
async function initWebcam() {
  if (webcamEnabled) return true;
  try {
    debugLog("Requesting webcam access...");
    webcamVideo = document.createElement("video");
    webcamVideo.setAttribute("autoplay", "");
    webcamVideo.setAttribute("playsinline", "");
    webcamCanvas = document.createElement("canvas");
    webcamCanvas.width = CAMERA_WIDTH;
    webcamCanvas.height = CAMERA_HEIGHT;
    webcamCtx = webcamCanvas.getContext("2d", { willReadFrequently: true });
    webcamStream = await navigator.mediaDevices.getUserMedia({
      video: {
        width: { ideal: 640 },
        height: { ideal: 480 },
        facingMode: "user"
      },
      audio: false
    });
    webcamVideo.srcObject = webcamStream;
    await webcamVideo.play();
    webcamEnabled = true;
    debugLog(`Webcam initialized: ${webcamVideo.videoWidth}x${webcamVideo.videoHeight}`);
    return true;
  } catch (err) {
    console.error("Failed to access webcam:", err);
    debugLog(`Webcam error: ${err.message}`);
    return false;
  }
}
function captureWebcamFrame() {
  if (!webcamEnabled || !webcamVideo || !emulator) return;
  const videoAspect = webcamVideo.videoWidth / webcamVideo.videoHeight;
  const targetAspect = CAMERA_WIDTH / CAMERA_HEIGHT;
  let srcX = 0, srcY = 0, srcW = webcamVideo.videoWidth, srcH = webcamVideo.videoHeight;
  if (videoAspect > targetAspect) {
    srcW = webcamVideo.videoHeight * targetAspect;
    srcX = (webcamVideo.videoWidth - srcW) / 2;
  } else {
    srcH = webcamVideo.videoWidth / targetAspect;
    srcY = (webcamVideo.videoHeight - srcH) / 2;
  }
  webcamCtx.save();
  webcamCtx.scale(-1, 1);
  webcamCtx.drawImage(
    webcamVideo,
    srcX,
    srcY,
    srcW,
    srcH,
    -CAMERA_WIDTH,
    0,
    CAMERA_WIDTH,
    CAMERA_HEIGHT
  );
  webcamCtx.restore();
  const imageData2 = webcamCtx.getImageData(0, 0, CAMERA_WIDTH, CAMERA_HEIGHT);
  const grayscale = new Uint8Array(CAMERA_WIDTH * CAMERA_HEIGHT);
  for (let i = 0; i < grayscale.length; i++) {
    const r = imageData2.data[i * 4];
    const g = imageData2.data[i * 4 + 1];
    const b = imageData2.data[i * 4 + 2];
    grayscale[i] = Math.round(0.299 * r + 0.587 * g + 0.114 * b);
  }
  emulator.set_camera_image(grayscale);
}
function downloadSave() {
  if (!emulator) {
    alert("No ROM loaded");
    return;
  }
  const saveData = emulator.get_cartridge_ram();
  const blob = new Blob([saveData], { type: "application/octet-stream" });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = `${currentRomName}.sav`;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
  debugLog(`Save file downloaded: ${currentRomName}.sav (${saveData.length} bytes)`);
}
async function loadSave(file) {
  if (!emulator) {
    alert("Load a ROM first before loading a save file");
    return;
  }
  const buffer = await file.arrayBuffer();
  const saveData = new Uint8Array(buffer);
  emulator.load_cartridge_ram(saveData);
  debugLog(`Save file loaded: ${file.name} (${saveData.length} bytes)`);
}
function setupSaveControls() {
  const downloadBtn = document.getElementById("download-save");
  const loadInput = document.getElementById("load-save");
  if (downloadBtn) {
    downloadBtn.addEventListener("click", downloadSave);
  }
  if (loadInput) {
    loadInput.addEventListener("change", async (e) => {
      const file = e.target.files[0];
      if (file) {
        await loadSave(file);
      }
      e.target.value = "";
    });
  }
}
async function initialize() {
  const wasm2 = await __wbg_init();
  wasmMemory = wasm2.memory;
  canvas = document.getElementById("screen");
  canvas.width = 160;
  canvas.height = 144;
  canvas.style.width = `${160 * SCALE}px`;
  canvas.style.height = `${144 * SCALE}px`;
  ctx = canvas.getContext("2d");
  imageData = ctx.createImageData(160, 144);
  for (let i = 0; i < imageData.data.length; i += 4) {
    imageData.data[i] = 155;
    imageData.data[i + 1] = 188;
    imageData.data[i + 2] = 15;
    imageData.data[i + 3] = 255;
  }
  ctx.putImageData(imageData, 0, 0);
  setupInputHandlers();
  setupFileInput();
  setupSaveControls();
  console.log("Emulator initialized");
}
function setupFileInput() {
  const input = document.getElementById("rom-input");
  input.addEventListener("change", async (e) => {
    const file = e.target.files[0];
    if (!file) return;
    debugLog(`Loading ROM file: ${file.name}`);
    stopEmulation();
    const buffer = await file.arrayBuffer();
    const romData = new Uint8Array(buffer);
    debugLog(`ROM file read: ${romData.length} bytes`);
    const headerBytes = Array.from(romData.slice(256, 272)).map((b) => b.toString(16).padStart(2, "0")).join(" ");
    debugLog(`ROM entry point bytes (0x100-0x10F): ${headerBytes}`);
    const title = String.fromCharCode(...romData.slice(308, 324)).replace(/\0/g, "");
    const cartType = romData[327];
    debugLog(`ROM title: "${title}", cart type: 0x${cartType.toString(16).padStart(2, "0")}`);
    try {
      debugLog("Creating GameBoy instance...");
      emulator = new GameBoy();
      debugLog("Loading ROM into emulator...");
      emulator.load_rom(romData);
      debugLog(`ROM loaded successfully: ${file.name} (${romData.length} bytes)`);
      currentRomName = file.name.replace(/\.(gb|gbc|bin)$/i, "");
      const saveControls = document.getElementById("save-controls");
      if (saveControls) {
        saveControls.style.display = "block";
      }
      isGameBoyCamera = cartType === 252;
      if (isGameBoyCamera) {
        debugLog("Game Boy Camera ROM detected - initializing webcam...");
        const webcamReady = await initWebcam();
        if (webcamReady) {
          debugLog("Webcam ready for Game Boy Camera");
          captureWebcamFrame();
        } else {
          debugLog("WARNING: Webcam not available - camera captures will be blank");
        }
      }
      debugLog("Calling log_vram_info...");
      emulator.log_vram_info();
      frameCounter = 0;
      startEmulation();
    } catch (err) {
      console.error("Failed to load ROM:", err);
      debugLog(`ERROR: Failed to load ROM: ${err}`);
      alert(`Failed to load ROM: ${err}`);
    }
  });
}
function setupInputHandlers() {
  const keyMap = {
    "ArrowRight": BUTTON_RIGHT,
    "ArrowLeft": BUTTON_LEFT,
    "ArrowUp": BUTTON_UP,
    "ArrowDown": BUTTON_DOWN,
    "KeyZ": BUTTON_A,
    "KeyX": BUTTON_B,
    "Enter": BUTTON_START,
    "ShiftLeft": BUTTON_SELECT,
    "ShiftRight": BUTTON_SELECT
  };
  document.addEventListener("keydown", (e) => {
    if (!emulator || !running) return;
    const button = keyMap[e.code];
    if (button !== void 0) {
      e.preventDefault();
      emulator.set_button(button, true);
    }
  });
  document.addEventListener("keyup", (e) => {
    if (!emulator) return;
    const button = keyMap[e.code];
    if (button !== void 0) {
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
  console.log("Emulation started");
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
    lastFrameTime = timestamp - elapsed % FRAME_DURATION;
    if (isGameBoyCamera && webcamEnabled && frameCounter % 4 === 0) {
      captureWebcamFrame();
    }
    emulator.step_frame();
    frameCounter++;
    const ptr = emulator.frame_buffer_ptr();
    const len = emulator.frame_buffer_len();
    const frameBuffer = new Uint8Array(wasmMemory.buffer, ptr, len);
    if (frameCounter <= 5 || frameCounter % 300 === 0) {
      let nonWhiteCount = 0;
      let uniqueColors = /* @__PURE__ */ new Set();
      for (let i = 0; i < frameBuffer.length; i += 4) {
        const r = frameBuffer[i];
        uniqueColors.add(r);
        if (r !== 255) {
          nonWhiteCount++;
        }
      }
      debugLog(`Frame ${frameCounter}: non-white pixels=${nonWhiteCount}, unique colors=${Array.from(uniqueColors).map((c) => "0x" + c.toString(16)).join(",")}`);
      emulator.get_debug_info();
    }
    imageData.data.set(frameBuffer);
    ctx.putImageData(imageData, 0, 0);
  }
  animationId = requestAnimationFrame(runFrame);
}
initialize().catch(console.error);
