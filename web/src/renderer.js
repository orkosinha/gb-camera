// ── Screen canvas + tile viewer ──────────────────────────────────────

export function createRenderer(state, screenCanvas, tileCanvas) {
    const screenCtx = screenCanvas.getContext('2d');
    const screenImageData = screenCtx.createImageData(160, 144);

    const tileCtx = tileCanvas.getContext('2d');
    const tileImageData = tileCtx.createImageData(128, 192);

    function renderScreen() {
        const ptr = state.emulator.frame_buffer_ptr();
        const len = state.emulator.frame_buffer_len();
        const buf = new Uint8ClampedArray(state.wasmMemory.buffer, ptr, len);
        screenImageData.data.set(buf);
        screenCtx.putImageData(screenImageData, 0, 0);
    }

    function renderTiles() {
        const tileData = state.emulator.read_range(0x8000, 384 * 16);
        const bgp = state.emulator.io_bgp();
        const palette = [
            (bgp >> 0) & 3,
            (bgp >> 2) & 3,
            (bgp >> 4) & 3,
            (bgp >> 6) & 3,
        ];
        const grays = [0xFF, 0xAA, 0x55, 0x00];
        const pixels = tileImageData.data;

        for (let tile = 0; tile < 384; tile++) {
            const tileX = (tile % 16) * 8;
            const tileY = Math.floor(tile / 16) * 8;
            const tileBase = tile * 16;

            for (let row = 0; row < 8; row++) {
                const low = tileData[tileBase + row * 2];
                const high = tileData[tileBase + row * 2 + 1];

                for (let col = 0; col < 8; col++) {
                    const bit = 7 - col;
                    const colorIdx = ((high >> bit) & 1) << 1 | ((low >> bit) & 1);
                    const gray = grays[palette[colorIdx]];
                    const px = tileX + col;
                    const py = tileY + row;
                    const i = (py * 128 + px) * 4;
                    pixels[i] = gray;
                    pixels[i + 1] = gray;
                    pixels[i + 2] = gray;
                    pixels[i + 3] = 255;
                }
            }
        }

        tileCtx.putImageData(tileImageData, 0, 0);
    }

    return { renderScreen, renderTiles };
}
