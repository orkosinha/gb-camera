// ── Panel updaters ──────────────────────────────────────────────────

import { hex2, hex4, escapeHtml } from './format.js';
import { disassemble } from './disassembler.js';

export function createPanels(state, dom) {
    const buttonState = {
        right: false, left: false, up: false, down: false,
        a: false, b: false, select: false, start: false,
    };

    const $ = id => document.getElementById(id);

    function updateCPU() {
        const e = state.emulator;
        const f = e.cpu_f();

        $('r-af').textContent = hex2(e.cpu_a()) + hex2(f);
        $('r-bc').textContent = hex4(e.cpu_bc());
        $('r-de').textContent = hex4(e.cpu_de());
        $('r-hl').textContent = hex4(e.cpu_hl());
        $('r-sp').textContent = hex4(e.cpu_sp());
        $('r-pc').textContent = hex4(e.cpu_pc());
        $('r-flags').textContent =
            ((f & 0x80) ? 'Z' : '-') +
            ((f & 0x40) ? 'N' : '-') +
            ((f & 0x20) ? 'H' : '-') +
            ((f & 0x10) ? 'C' : '-');
        $('r-ime').textContent = e.cpu_ime() ? '1' : '0';
        $('r-halt').textContent = e.cpu_halted() ? '1' : '0';
    }

    function drawPaletteCanvas(canvasId, getFn) {
        const canvas = $(canvasId);
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        const img = ctx.createImageData(32, 1);
        for (let pal = 0; pal < 8; pal++) {
            for (let col = 0; col < 4; col++) {
                const rgb = getFn(pal, col);
                const i = (pal * 4 + col) * 4;
                img.data[i]     = (rgb >> 16) & 0xFF; // R
                img.data[i + 1] = (rgb >>  8) & 0xFF; // G
                img.data[i + 2] =  rgb        & 0xFF; // B
                img.data[i + 3] = 255;
            }
        }
        ctx.putImageData(img, 0, 0);
    }

    function updatePPU() {
        const e = state.emulator;
        const mode = e.ppu_mode();
        const modes = ['HBL', 'VBL', 'OAM', 'DRW'];
        const cgb = e.is_cgb_mode();

        $('r-ly').textContent = String(e.ppu_line()).padStart(3);
        $('r-mode').textContent = `${mode} ${modes[mode] || '?'}`;
        $('r-lcdc').textContent = hex2(e.io_lcdc());
        $('r-stat').textContent = hex2(e.io_stat());
        $('r-scx').textContent = hex2(e.io_scx());
        $('r-scy').textContent = hex2(e.io_scy());
        $('r-wx').textContent = hex2(e.io_wx());
        $('r-wy').textContent = hex2(e.io_wy());

        // Mode badge + conditional rows
        $('cgb-badge').style.display = cgb ? '' : 'none';
        $('row-dmg-pal').style.display = cgb ? 'none' : '';
        $('row-cgb-regs').style.display = cgb ? '' : 'none';
        $('cgb-palettes').style.display = cgb ? '' : 'none';

        if (cgb) {
            $('r-vbk').textContent  = String(e.io_vbk());
            $('r-svbk').textContent = String(e.io_svbk());
            $('r-key1').textContent = hex2(e.io_key1());
            drawPaletteCanvas('cgb-bg-pal',  (p, c) => e.get_bg_palette_color(p, c));
            drawPaletteCanvas('cgb-obj-pal', (p, c) => e.get_obj_palette_color(p, c));
        } else {
            $('r-bgp').textContent  = hex2(e.io_bgp());
            $('r-obp0').textContent = hex2(e.io_obp0());
            $('r-obp1').textContent = hex2(e.io_obp1());
        }
    }

    function updateDisassembly() {
        const e = state.emulator;
        const pc = e.cpu_pc();
        const read = addr => e.read_byte(addr & 0xFFFF);
        const lines = disassemble(read, pc, 16);

        let html = '';
        for (const l of lines) {
            const text = `${hex4(l.addr)} ${l.rawBytes} ${l.text}`;
            html += l.addr === pc
                ? `<span class="pc">${escapeHtml(text)}</span>\n`
                : escapeHtml(text) + '\n';
        }
        dom.disPre.innerHTML = html;
    }

    function updateMemory() {
        const e = state.emulator;
        const base = state.memViewAddr & 0xFFF0;
        const data = e.read_range(base, 256);

        let text = '      00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F\n';
        for (let row = 0; row < 16; row++) {
            const addr = (base + row * 16) & 0xFFFF;
            let line = hex4(addr) + ' ';
            let ascii = '';
            for (let col = 0; col < 16; col++) {
                const b = data[row * 16 + col];
                line += hex2(b) + (col === 7 ? '  ' : ' ');
                ascii += (b >= 0x20 && b <= 0x7E) ? String.fromCharCode(b) : '.';
            }
            text += line + ascii + '\n';
        }
        dom.memPre.textContent = text;
    }

    function updateTimer() {
        const e = state.emulator;
        const tac = e.io_tac();

        $('r-div').textContent = hex2(e.io_div());
        $('r-tima').textContent = hex2(e.io_tima());
        $('r-tma').textContent = hex2(e.io_tma());
        $('r-tac').textContent = hex2(tac);
    }

    function updateInterrupts() {
        const e = state.emulator;
        const ie = e.io_ie(), ifl = e.io_if();

        $('r-ie').textContent = hex2(ie);
        $('r-if').textContent = hex2(ifl);

        const ids = ['vbl', 'lcd', 'tim', 'ser', 'joy'];
        for (let i = 0; i < 5; i++) {
            const en = (ie >> i) & 1, fl = (ifl >> i) & 1;
            // * = enabled+pending, + = enabled, ! = pending, - = off
            $(`r-int-${ids[i]}`).textContent = en ? (fl ? '*' : '+') : (fl ? '!' : '-');
        }
    }

    function updateSerial() {
        const text = state.emulator.get_serial_output();
        if (text) dom.serialPre.textContent = text;
    }

    function updateAll() {
        updateCPU();
        updatePPU();
        updateDisassembly();
        updateMemory();
        updateTimer();
        updateInterrupts();
        updateSerial();
    }

    return { updateAll, buttonState };
}
