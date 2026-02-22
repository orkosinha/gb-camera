// ── Screen canvas + tile viewer + BG map ─────────────────────────────

export function createRenderer(state, screenCanvas, tileCanvas, bgMapCanvas) {
    const screenCtx = screenCanvas.getContext('2d');
    const screenImageData = screenCtx.createImageData(160, 144);

    const tileCtx = tileCanvas.getContext('2d');
    const tileImageData = tileCtx.createImageData(128, 192);

    const bgMapCtx     = bgMapCanvas ? bgMapCanvas.getContext('2d') : null;
    const bgMapImgData = bgMapCtx ? bgMapCtx.createImageData(256, 256) : null;

    function renderScreen() {
        const ptr = state.emulator.frame_buffer_ptr();
        const len = state.emulator.frame_buffer_len();
        const buf = new Uint8ClampedArray(state.wasmMemory.buffer, ptr, len);
        screenImageData.data.set(buf);
        screenCtx.putImageData(screenImageData, 0, 0);
    }

    // Decode one 8-pixel tile row from two planar bytes into color indices 0–3.
    function decodeTileRow(low, high) {
        const out = new Uint8Array(8);
        for (let col = 0; col < 8; col++) {
            const bit = 7 - col;
            out[col] = (((high >> bit) & 1) << 1) | ((low >> bit) & 1);
        }
        return out;
    }

    function renderTiles(bank = 0) {
        const e = state.emulator;
        const cgb = e.is_cgb_mode();
        const tileData = e.read_vram_bank(bank & 1, 0x8000, 384 * 16);
        const pixels = tileImageData.data;

        if (cgb) {
            // CGB: render all tiles using BG palette 0
            for (let tile = 0; tile < 384; tile++) {
                const tileX = (tile % 16) * 8;
                const tileY = Math.floor(tile / 16) * 8;
                const tileBase = tile * 16;
                for (let row = 0; row < 8; row++) {
                    const low  = tileData[tileBase + row * 2];
                    const high = tileData[tileBase + row * 2 + 1];
                    const ci   = decodeTileRow(low, high);
                    for (let col = 0; col < 8; col++) {
                        const rgb = e.get_bg_palette_color(0, ci[col]);
                        const i = ((tileY + row) * 128 + tileX + col) * 4;
                        pixels[i]     = (rgb >> 16) & 0xFF;
                        pixels[i + 1] = (rgb >>  8) & 0xFF;
                        pixels[i + 2] =  rgb        & 0xFF;
                        pixels[i + 3] = 255;
                    }
                }
            }
        } else {
            // DMG: grayscale via BGP
            const bgp = e.io_bgp();
            const pal   = [(bgp >> 0) & 3, (bgp >> 2) & 3, (bgp >> 4) & 3, (bgp >> 6) & 3];
            const grays = [0xFF, 0xAA, 0x55, 0x00];
            for (let tile = 0; tile < 384; tile++) {
                const tileX = (tile % 16) * 8;
                const tileY = Math.floor(tile / 16) * 8;
                const tileBase = tile * 16;
                for (let row = 0; row < 8; row++) {
                    const low  = tileData[tileBase + row * 2];
                    const high = tileData[tileBase + row * 2 + 1];
                    const ci   = decodeTileRow(low, high);
                    for (let col = 0; col < 8; col++) {
                        const gray = grays[pal[ci[col]]];
                        const i = ((tileY + row) * 128 + tileX + col) * 4;
                        pixels[i] = pixels[i + 1] = pixels[i + 2] = gray;
                        pixels[i + 3] = 255;
                    }
                }
            }
        }

        tileCtx.putImageData(tileImageData, 0, 0);
    }

    function renderBgMap() {
        if (!bgMapCtx || !bgMapImgData) return;

        const e    = state.emulator;
        const lcdc = e.io_lcdc();
        const scx  = e.io_scx();
        const scy  = e.io_scy();
        const cgb  = e.is_cgb_mode();

        // Tilemap base: LCDC bit 3 selects 0x9C00 vs 0x9800
        const mapBase   = (lcdc & 0x08) ? 0x9C00 : 0x9800;
        // Tile data addressing: LCDC bit 4 = 0 → signed (base 0x8800), bit 4 = 1 → unsigned (base 0x8000)
        const signedAddr = (lcdc & 0x10) === 0;

        const tileMap  = e.read_vram_bank(0, mapBase, 1024);
        const attrMap  = cgb ? e.read_vram_bank(1, mapBase, 1024) : null;
        const tileDataB0 = e.read_vram_bank(0, 0x8000, 6144);
        const tileDataB1 = cgb ? e.read_vram_bank(1, 0x8000, 6144) : null;

        // Pre-decode DMG palette once
        const bgp = e.io_bgp();
        const dmgPal   = [(bgp >> 0) & 3, (bgp >> 2) & 3, (bgp >> 4) & 3, (bgp >> 6) & 3];
        const dmgGrays = [0xFF, 0xAA, 0x55, 0x00];

        const pixels = bgMapImgData.data;

        for (let tileRow = 0; tileRow < 32; tileRow++) {
            for (let tileCol = 0; tileCol < 32; tileCol++) {
                const mapIdx = tileRow * 32 + tileCol;
                const rawIdx = tileMap[mapIdx];

                // Resolve tile data offset (bytes from 0x8000)
                let tileOff;
                if (signedAddr) {
                    // Signed: 0x00–0x7F → tiles at 0x9000–0x97F0
                    //         0x80–0xFF → tiles at 0x8800–0x8FF0
                    const s = rawIdx < 128 ? rawIdx : rawIdx - 256;
                    tileOff = (s + 256) * 16;
                } else {
                    tileOff = rawIdx * 16;
                }

                // CGB attributes (bank 1)
                let cgbPalette = 0, tileBank = 0, xFlip = false, yFlip = false;
                if (cgb && attrMap) {
                    const attr = attrMap[mapIdx];
                    cgbPalette = attr & 0x07;
                    tileBank   = (attr >> 3) & 1;
                    xFlip      = (attr & 0x20) !== 0;
                    yFlip      = (attr & 0x40) !== 0;
                }

                const tileData = (cgb && tileBank === 1) ? tileDataB1 : tileDataB0;

                for (let py = 0; py < 8; py++) {
                    const srcRow = yFlip ? (7 - py) : py;
                    const low    = tileData[tileOff + srcRow * 2];
                    const high   = tileData[tileOff + srcRow * 2 + 1];
                    const ci     = decodeTileRow(low, high);

                    for (let px = 0; px < 8; px++) {
                        const colorIdx = ci[xFlip ? (7 - px) : px];
                        const destI = ((tileRow * 8 + py) * 256 + tileCol * 8 + px) * 4;

                        if (cgb) {
                            const rgb = e.get_bg_palette_color(cgbPalette, colorIdx);
                            pixels[destI]     = (rgb >> 16) & 0xFF;
                            pixels[destI + 1] = (rgb >>  8) & 0xFF;
                            pixels[destI + 2] =  rgb        & 0xFF;
                        } else {
                            const gray = dmgGrays[dmgPal[colorIdx]];
                            pixels[destI] = pixels[destI + 1] = pixels[destI + 2] = gray;
                        }
                        pixels[destI + 3] = 255;
                    }
                }
            }
        }

        bgMapCtx.putImageData(bgMapImgData, 0, 0);

        // Viewport rectangle (red dashed) — shows the 160×144 screen window
        bgMapCtx.save();
        bgMapCtx.strokeStyle = 'red';
        bgMapCtx.lineWidth   = 1;
        bgMapCtx.setLineDash([4, 3]);
        bgMapCtx.strokeRect(scx + 0.5, scy + 0.5, 160, 144);
        bgMapCtx.restore();
    }

    return { renderScreen, renderTiles, renderBgMap };
}
