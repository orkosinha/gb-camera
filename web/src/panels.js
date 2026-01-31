// ── CPU/PPU/Memory/Timer/Interrupt/Joypad/Serial panel updaters ──────

import { hex2, hex4, escapeHtml } from './format.js';
import { disassemble } from './disassembler.js';

export function createPanels(state, domRefs) {
    const { cpuPre, ppuPre, disPre, memPre, timerPre, intPre, serialPre } = domRefs;

    const buttonState = {
        right: false, left: false, up: false, down: false,
        a: false, b: false, select: false, start: false,
    };

    function updateCPU() {
        const emu = state.emulator;
        const a = emu.cpu_a(), f = emu.cpu_f();
        const bc = emu.cpu_bc(), de = emu.cpu_de(), hl = emu.cpu_hl();
        const sp = emu.cpu_sp(), pc = emu.cpu_pc();
        const ime = emu.cpu_ime(), halted = emu.cpu_halted();

        const z = (f & 0x80) ? 'Z' : '-';
        const n = (f & 0x40) ? 'N' : '-';
        const h = (f & 0x20) ? 'H' : '-';
        const c = (f & 0x10) ? 'C' : '-';

        cpuPre.textContent =
            `AF  ${hex2(a)}${hex2(f)}   Flags ${z}${n}${h}${c}\n` +
            `BC  ${hex4(bc)}\n` +
            `DE  ${hex4(de)}\n` +
            `HL  ${hex4(hl)}\n` +
            `SP  ${hex4(sp)}\n` +
            `PC  ${hex4(pc)}\n` +
            `\n` +
            `IME ${ime ? 'ON ' : 'OFF'}   HALT ${halted ? 'YES' : 'NO '}`;
    }

    function updatePPU() {
        const emu = state.emulator;
        const mode = emu.ppu_mode();
        const line = emu.ppu_line();
        const cycles = emu.ppu_cycles();
        const lcdc = emu.io_lcdc();
        const stat = emu.io_stat();
        const scy = emu.io_scy(), scx = emu.io_scx();
        const wy = emu.io_wy(), wx = emu.io_wx();
        const bgp = emu.io_bgp();
        const obp0 = emu.io_obp0(), obp1 = emu.io_obp1();

        const modeNames = ['HBLANK','VBLANK','OAM','DRAW'];
        const modeName = (modeNames[mode] || '?').padEnd(6);

        ppuPre.textContent =
            `Mode   ${mode} ${modeName}  LY ${String(line).padStart(3)}\n` +
            `Cycles ${String(cycles).padStart(5)}\n` +
            `LCDC   ${hex2(lcdc)}   STAT ${hex2(stat)}\n` +
            `  LCD ${(lcdc>>7)&1}  BG  ${lcdc&1}  Win ${(lcdc>>5)&1}\n` +
            `  OBJ ${(lcdc>>1)&1}  sz  ${(lcdc>>2)&1?'8x16':'8x8 '}\n` +
            `  Dat ${(lcdc>>4)&1?'$8000':'$8800'}  BGm ${(lcdc>>3)&1?'$9C00':'$9800'}\n` +
            `  Wnm ${(lcdc>>6)&1?'$9C00':'$9800'}\n` +
            `SCY ${hex2(scy)}  SCX ${hex2(scx)}\n` +
            `WY  ${hex2(wy)}  WX  ${hex2(wx)}\n` +
            `BGP ${hex2(bgp)}  OBP0 ${hex2(obp0)}  OBP1 ${hex2(obp1)}`;
    }

    function updateDisassembly() {
        const emu = state.emulator;
        const pc = emu.cpu_pc();
        const readByte = (addr) => emu.read_byte(addr & 0xFFFF);
        const lines = disassemble(readByte, pc, 24);

        let html = '';
        for (let i = 0; i < lines.length; i++) {
            const l = lines[i];
            const prefix = (l.addr === pc) ? '\u25B6 ' : '  ';
            const line = `${prefix}${hex4(l.addr)}  ${l.rawBytes}  ${l.text}`;
            if (l.addr === pc) {
                html += `<span class="current-pc">${escapeHtml(line)}</span>\n`;
            } else {
                html += escapeHtml(line) + '\n';
            }
        }
        disPre.innerHTML = html;
    }

    function updateMemory() {
        const emu = state.emulator;
        const base = state.memViewAddr & 0xFFF0;
        const data = emu.read_range(base, 256);
        let text = '       00 01 02 03 04 05 06 07  08 09 0A 0B 0C 0D 0E 0F\n';
        text += '       -- -- -- -- -- -- -- --  -- -- -- -- -- -- -- --\n';
        for (let row = 0; row < 16; row++) {
            const addr = (base + row * 16) & 0xFFFF;
            let line = hex4(addr) + '  ';
            let ascii = '';
            for (let col = 0; col < 16; col++) {
                const b = data[row * 16 + col];
                line += hex2(b);
                if (col === 7) line += '  '; else if (col < 15) line += ' ';
                ascii += (b >= 0x20 && b <= 0x7E) ? String.fromCharCode(b) : '.';
            }
            text += line + '  ' + ascii + '\n';
        }
        memPre.textContent = text;
    }

    function updateTimer() {
        const emu = state.emulator;
        const div = emu.io_div();
        const tima = emu.io_tima();
        const tma = emu.io_tma();
        const tac = emu.io_tac();

        const tacEnabled = (tac & 0x04) ? 'ON' : 'OFF';
        const tacClock = ['4096','262144','65536','16384'][tac & 0x03];

        timerPre.textContent =
            `DIV  ${hex2(div)}   TIMA ${hex2(tima)}\n` +
            `TMA  ${hex2(tma)}   TAC  ${hex2(tac)}\n` +
            `En   ${tacEnabled.padEnd(3)}  Clk  ${tacClock} Hz`;
    }

    function updateInterrupts() {
        const emu = state.emulator;
        const ie = emu.io_ie();
        const ifl = emu.io_if();
        const names = ['VBlank','LCD','Timer','Serial','Joypad'];

        let text = '         IE IF\n';
        for (let i = 0; i < 5; i++) {
            const e = (ie >> i) & 1;
            const f = (ifl >> i) & 1;
            text += `${names[i].padEnd(9)}${e}  ${f}\n`;
        }
        intPre.textContent = text;
    }

    function updateJoypad() {
        const ids = { up:'jp-up', down:'jp-down', left:'jp-left', right:'jp-right',
                      a:'jp-a', b:'jp-b', select:'jp-select', start:'jp-start' };
        for (const [key, id] of Object.entries(ids)) {
            const el = document.getElementById(id);
            if (el) el.classList.toggle('pressed', buttonState[key]);
        }
    }

    function updateSerial() {
        const text = state.emulator.get_serial_output();
        if (text.length > 0) {
            serialPre.textContent = text;
        }
    }

    function updateAll() {
        updateCPU();
        updatePPU();
        updateDisassembly();
        updateMemory();
        updateTimer();
        updateInterrupts();
        updateJoypad();
        updateSerial();
    }

    return { updateAll, buttonState };
}
