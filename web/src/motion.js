// ── MBC7 accelerometer / tilt input ─────────────────────────────────────────
//
// Wraps the DeviceMotion API and exposes a simple enable/disable interface.
//
// Scale mapping:
//   DeviceMotion gives accelerationIncludingGravity in m/s².
//   Flat device: x ≈ 0, y ≈ 0, z ≈ -9.8.
//   We convert to MBC7 units: ±0x1000 per g (per 9.8 m/s²).
//   X axis: tilt right = positive device-x → positive MBC7 x (ball rolls right).
//   Y axis: tilt forward (screen-down) = negative device-y → positive MBC7 y (ball rolls up).
//
// Desktop fallback: drag on the screen canvas to simulate tilt.

const SCALE     = 0x1000 / 9.807; // MBC7 units per m/s²
const MAX       = 0x2000;          // clamp at ±2g — matches ADXL202E hardware range
const ALPHA     = 0.25;            // EMA smoothing factor (lower = smoother, more lag)
const DEAD_ZONE = 0x1000 * 0.10;  // ignore tilts < ~0.1g to prevent idle drift

export function createMotion(onTilt) {
    let enabled    = false;
    let usingMouse = false;
    let handler    = null;
    let filtX = 0, filtY = 0;     // low-pass filter state

    // ── DeviceMotion (mobile) ────────────────────────────────────────────────

    async function requestPermission() {
        if (typeof DeviceMotionEvent === 'undefined') return false;

        // iOS 13+ requires an explicit user-gesture permission request.
        if (typeof DeviceMotionEvent.requestPermission === 'function') {
            try {
                const result = await DeviceMotionEvent.requestPermission();
                return result === 'granted';
            } catch {
                return false;
            }
        }

        // Desktop browsers define DeviceMotionEvent but have no real sensor.
        // Use touch-point count as a reliable proxy for "real mobile device".
        if (navigator.maxTouchPoints === 0) return false;

        return true; // Android and other touch devices
    }

    function motionHandler(e) {
        const ag = e.accelerationIncludingGravity;
        if (!ag) return;

        // Exponential moving average — smooths out sensor noise
        filtX += ALPHA * ( (ag.x ?? 0) * SCALE - filtX);
        filtY += ALPHA * (-(ag.y ?? 0) * SCALE - filtY);

        // Dead zone — snap to zero when below threshold (prevents idle drift)
        const x = Math.abs(filtX) < DEAD_ZONE ? 0 : filtX;
        const y = Math.abs(filtY) < DEAD_ZONE ? 0 : filtY;

        onTilt(
            Math.max(-MAX, Math.min(MAX, Math.round(x))),
            Math.max(-MAX, Math.min(MAX, Math.round(y))),
        );
    }

    async function enableMotion() {
        if (!(await requestPermission())) return false;
        filtX = 0;
        filtY = 0;
        handler = motionHandler;
        window.addEventListener('devicemotion', handler);
        enabled = true;
        usingMouse = false;
        return true;
    }

    // ── Mouse drag fallback (desktop) ────────────────────────────────────────
    // Dragging from the canvas centre simulates tilt angle.

    function enableMouse(canvas) {
        let dragging = false;
        let cx = 0, cy = 0;

        canvas.addEventListener('mousedown', e => {
            dragging = true;
            const r = canvas.getBoundingClientRect();
            cx = r.left + r.width  / 2;
            cy = r.top  + r.height / 2;
        });
        window.addEventListener('mousemove', e => {
            if (!dragging) return;
            // Map ±half-canvas-width to ±MAX
            const r   = canvas.getBoundingClientRect();
            const half = Math.max(r.width, r.height) / 2;
            const x = Math.round(((e.clientX - cx) / half) * MAX);
            const y = Math.round(((e.clientY - cy) / half) * MAX);
            onTilt(
                Math.max(-MAX, Math.min(MAX, x)),
                Math.max(-MAX, Math.min(MAX, y)),
            );
        });
        window.addEventListener('mouseup', () => {
            if (dragging) {
                dragging = false;
                onTilt(0, 0); // return to centre on release
            }
        });
        enabled    = true;
        usingMouse = true;
    }

    function disable() {
        if (handler) {
            window.removeEventListener('devicemotion', handler);
            handler = null;
        }
        filtX   = 0;
        filtY   = 0;
        enabled    = false;
        usingMouse = false;
        onTilt(0, 0);
    }

    return { enableMotion, enableMouse, disable, isEnabled: () => enabled, isMouse: () => usingMouse };
}
