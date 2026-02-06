// ── Webcam lifecycle, live view, gallery ─────────────────────────────

const CAMERA_WIDTH = 128;
const CAMERA_HEIGHT = 112;
const SMOOTHING_FACTOR = 0.7;

export function createCamera(state, domRefs) {
    let webcamStream = null;
    let webcamVideo = null;
    let webcamCanvas = null;
    let webcamCtx = null;
    let webcamEnabled = false;
    let previousGrayscale = null;
    let liveCtx = null;
    let liveImageData = null;
    let webcamPreviewCtx = null;
    let galleryInterval = null;

    async function initWebcam() {
        if (webcamEnabled) return true;

        try {
            webcamVideo = document.createElement('video');
            webcamVideo.setAttribute('autoplay', '');
            webcamVideo.setAttribute('playsinline', '');

            webcamCanvas = document.createElement('canvas');
            webcamCanvas.width = CAMERA_WIDTH;
            webcamCanvas.height = CAMERA_HEIGHT;
            webcamCtx = webcamCanvas.getContext('2d', { willReadFrequently: true });

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
            previousGrayscale = null;

            const statusEl = domRefs.webcamStatus;
            if (statusEl) statusEl.textContent = `${webcamVideo.videoWidth}x${webcamVideo.videoHeight}`;

            const btnEl = domRefs.btnWebcamToggle;
            if (btnEl) btnEl.textContent = 'Disable Webcam';

            return true;
        } catch (err) {
            console.error('Failed to access webcam:', err);
            const statusEl = domRefs.webcamStatus;
            if (statusEl) statusEl.textContent = `Error: ${err.message}`;
            return false;
        }
    }

    function stopWebcam() {
        if (webcamStream) {
            webcamStream.getTracks().forEach(track => track.stop());
            webcamStream = null;
        }
        webcamEnabled = false;
        previousGrayscale = null;

        const statusEl = domRefs.webcamStatus;
        if (statusEl) statusEl.textContent = 'Disabled';

        const btnEl = domRefs.btnWebcamToggle;
        if (btnEl) btnEl.textContent = 'Enable Webcam';
    }

    function captureFrame() {
        if (!webcamEnabled || !webcamVideo || !state.emulator) return;

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

        // Mirror horizontally for selfie view
        webcamCtx.save();
        webcamCtx.scale(-1, 1);
        webcamCtx.drawImage(
            webcamVideo,
            srcX, srcY, srcW, srcH,
            -CAMERA_WIDTH, 0, CAMERA_WIDTH, CAMERA_HEIGHT
        );
        webcamCtx.restore();

        // Also draw to the preview canvas
        if (webcamPreviewCtx) {
            webcamPreviewCtx.save();
            webcamPreviewCtx.scale(-1, 1);
            webcamPreviewCtx.drawImage(
                webcamVideo,
                srcX, srcY, srcW, srcH,
                -CAMERA_WIDTH, 0, CAMERA_WIDTH, CAMERA_HEIGHT
            );
            webcamPreviewCtx.restore();
        }

        const imageData = webcamCtx.getImageData(0, 0, CAMERA_WIDTH, CAMERA_HEIGHT);
        const grayscale = new Uint8Array(CAMERA_WIDTH * CAMERA_HEIGHT);

        for (let i = 0; i < grayscale.length; i++) {
            const r = imageData.data[i * 4];
            const g = imageData.data[i * 4 + 1];
            const b = imageData.data[i * 4 + 2];
            let value = 0.299 * r + 0.587 * g + 0.114 * b;

            if (previousGrayscale) {
                value = SMOOTHING_FACTOR * value + (1 - SMOOTHING_FACTOR) * previousGrayscale[i];
            }
            grayscale[i] = Math.round(value);
        }

        previousGrayscale = grayscale;
        state.emulator.set_camera_image(grayscale);
    }

    function updateLiveView() {
        if (!state.emulator || !liveCtx) return;
        if (!state.emulator.update_camera_live()) return;

        const ptr = state.emulator.camera_live_ptr();
        const len = state.emulator.camera_live_len();
        const rgba = new Uint8Array(state.wasmMemory.buffer, ptr, len);

        if (!liveImageData) {
            liveImageData = liveCtx.createImageData(128, 112);
        }
        liveImageData.data.set(rgba);
        liveCtx.putImageData(liveImageData, 0, 0);

        updateSettings();
    }

    function updateSettings() {
        const el = domRefs.cameraSettings;
        if (!el || !state.emulator) return;

        // Camera registers: A001=config, A002=exposure lo, A003=exposure hi, A005=voltage offset
        const reg1 = state.emulator.camera_reg(1);
        const expLo = state.emulator.camera_reg(2);
        const expHi = state.emulator.camera_reg(3);
        const offset = state.emulator.camera_reg(5);
        const exposure = (expHi << 8) | expLo;
        const gain = (reg1 >> 4) & 0x03;
        const contrast = state.emulator.camera_contrast();
        const contrastStr = contrast >= 0 ? `${contrast}/15` : '?';

        el.textContent = `Exposure: 0x${exposure.toString(16).toUpperCase().padStart(4, '0')}  Contrast: ${contrastStr}  Gain: ${gain}  Offset: ${offset}`;
    }

    function updateGallery() {
        if (!state.emulator) return;
        const grid = domRefs.galleryGrid;
        if (!grid) return;

        grid.innerHTML = '';

        for (let slot = 1; slot <= 30; slot++) {
            const rgba = state.emulator.decode_camera_photo(slot);
            if (rgba.length === 0) continue;

            const container = document.createElement('div');
            container.className = 'gallery-slot';

            const label = document.createElement('div');
            label.textContent = `#${slot}`;

            const c = document.createElement('canvas');
            c.width = 128;
            c.height = 112;
            c.style.cssText = 'width:64px;height:56px;';

            container.appendChild(label);
            container.appendChild(c);
            grid.appendChild(container);

            const ctx2 = c.getContext('2d');
            const img = new ImageData(
                new Uint8ClampedArray(rgba.buffer, rgba.byteOffset, rgba.byteLength),
                128, 112
            );
            ctx2.putImageData(img, 0, 0);
        }
    }

    function startCamera() {
        const statusEl = domRefs.cameraStatus;
        if (statusEl) statusEl.textContent = 'Camera ROM detected';

        const liveCanvas = domRefs.liveCapture;
        if (liveCanvas) liveCtx = liveCanvas.getContext('2d');

        const previewCanvas = domRefs.webcamPreview;
        if (previewCanvas) webcamPreviewCtx = previewCanvas.getContext('2d');

        updateGallery();
        galleryInterval = setInterval(updateGallery, 2000);
    }

    function stopCamera() {
        if (galleryInterval) {
            clearInterval(galleryInterval);
            galleryInterval = null;
        }
        liveCtx = null;
        liveImageData = null;
        webcamPreviewCtx = null;
        const statusEl = domRefs.cameraStatus;
        if (statusEl) statusEl.textContent = 'No camera ROM loaded';
    }

    function isWebcamEnabled() {
        return webcamEnabled;
    }

    return {
        initWebcam, stopWebcam, captureFrame,
        updateLiveView, startCamera, stopCamera, isWebcamEnabled,
    };
}
