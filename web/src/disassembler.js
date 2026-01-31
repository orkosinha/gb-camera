// ── Disassembler — opcode tables + disassemble() ────────────────────

import { hex2, hex4 } from './format.js';

const OPCODE_TABLE = buildOpcodeTable();
const CB_TABLE = buildCBTable();

function buildOpcodeTable() {
    const t = new Array(256);
    for (let i = 0; i < 256; i++) t[i] = { m: `DB $${hex2(i)}`, l: 1 };

    t[0x00] = { m: 'NOP', l: 1 };
    t[0x01] = { m: 'LD BC,d16', l: 3 };
    t[0x02] = { m: 'LD (BC),A', l: 1 };
    t[0x03] = { m: 'INC BC', l: 1 };
    t[0x04] = { m: 'INC B', l: 1 };
    t[0x05] = { m: 'DEC B', l: 1 };
    t[0x06] = { m: 'LD B,d8', l: 2 };
    t[0x07] = { m: 'RLCA', l: 1 };
    t[0x08] = { m: 'LD (a16),SP', l: 3 };
    t[0x09] = { m: 'ADD HL,BC', l: 1 };
    t[0x0A] = { m: 'LD A,(BC)', l: 1 };
    t[0x0B] = { m: 'DEC BC', l: 1 };
    t[0x0C] = { m: 'INC C', l: 1 };
    t[0x0D] = { m: 'DEC C', l: 1 };
    t[0x0E] = { m: 'LD C,d8', l: 2 };
    t[0x0F] = { m: 'RRCA', l: 1 };

    t[0x10] = { m: 'STOP', l: 2 };
    t[0x11] = { m: 'LD DE,d16', l: 3 };
    t[0x12] = { m: 'LD (DE),A', l: 1 };
    t[0x13] = { m: 'INC DE', l: 1 };
    t[0x14] = { m: 'INC D', l: 1 };
    t[0x15] = { m: 'DEC D', l: 1 };
    t[0x16] = { m: 'LD D,d8', l: 2 };
    t[0x17] = { m: 'RLA', l: 1 };
    t[0x18] = { m: 'JR r8', l: 2 };
    t[0x19] = { m: 'ADD HL,DE', l: 1 };
    t[0x1A] = { m: 'LD A,(DE)', l: 1 };
    t[0x1B] = { m: 'DEC DE', l: 1 };
    t[0x1C] = { m: 'INC E', l: 1 };
    t[0x1D] = { m: 'DEC E', l: 1 };
    t[0x1E] = { m: 'LD E,d8', l: 2 };
    t[0x1F] = { m: 'RRA', l: 1 };

    t[0x20] = { m: 'JR NZ,r8', l: 2 };
    t[0x21] = { m: 'LD HL,d16', l: 3 };
    t[0x22] = { m: 'LD (HL+),A', l: 1 };
    t[0x23] = { m: 'INC HL', l: 1 };
    t[0x24] = { m: 'INC H', l: 1 };
    t[0x25] = { m: 'DEC H', l: 1 };
    t[0x26] = { m: 'LD H,d8', l: 2 };
    t[0x27] = { m: 'DAA', l: 1 };
    t[0x28] = { m: 'JR Z,r8', l: 2 };
    t[0x29] = { m: 'ADD HL,HL', l: 1 };
    t[0x2A] = { m: 'LD A,(HL+)', l: 1 };
    t[0x2B] = { m: 'DEC HL', l: 1 };
    t[0x2C] = { m: 'INC L', l: 1 };
    t[0x2D] = { m: 'DEC L', l: 1 };
    t[0x2E] = { m: 'LD L,d8', l: 2 };
    t[0x2F] = { m: 'CPL', l: 1 };

    t[0x30] = { m: 'JR NC,r8', l: 2 };
    t[0x31] = { m: 'LD SP,d16', l: 3 };
    t[0x32] = { m: 'LD (HL-),A', l: 1 };
    t[0x33] = { m: 'INC SP', l: 1 };
    t[0x34] = { m: 'INC (HL)', l: 1 };
    t[0x35] = { m: 'DEC (HL)', l: 1 };
    t[0x36] = { m: 'LD (HL),d8', l: 2 };
    t[0x37] = { m: 'SCF', l: 1 };
    t[0x38] = { m: 'JR C,r8', l: 2 };
    t[0x39] = { m: 'ADD HL,SP', l: 1 };
    t[0x3A] = { m: 'LD A,(HL-)', l: 1 };
    t[0x3B] = { m: 'DEC SP', l: 1 };
    t[0x3C] = { m: 'INC A', l: 1 };
    t[0x3D] = { m: 'DEC A', l: 1 };
    t[0x3E] = { m: 'LD A,d8', l: 2 };
    t[0x3F] = { m: 'CCF', l: 1 };

    const regs = ['B','C','D','E','H','L','(HL)','A'];
    for (let d = 0; d < 8; d++) {
        for (let s = 0; s < 8; s++) {
            const op = 0x40 + d * 8 + s;
            if (op === 0x76) { t[op] = { m: 'HALT', l: 1 }; continue; }
            t[op] = { m: `LD ${regs[d]},${regs[s]}`, l: 1 };
        }
    }

    const alu = ['ADD A,','ADC A,','SUB','SBC A,','AND','XOR','OR','CP'];
    for (let a = 0; a < 8; a++) {
        for (let s = 0; s < 8; s++) {
            t[0x80 + a * 8 + s] = { m: `${alu[a]} ${regs[s]}`, l: 1 };
        }
    }

    t[0xC0] = { m: 'RET NZ', l: 1 };
    t[0xC1] = { m: 'POP BC', l: 1 };
    t[0xC2] = { m: 'JP NZ,a16', l: 3 };
    t[0xC3] = { m: 'JP a16', l: 3 };
    t[0xC4] = { m: 'CALL NZ,a16', l: 3 };
    t[0xC5] = { m: 'PUSH BC', l: 1 };
    t[0xC6] = { m: 'ADD A,d8', l: 2 };
    t[0xC7] = { m: 'RST 00H', l: 1 };
    t[0xC8] = { m: 'RET Z', l: 1 };
    t[0xC9] = { m: 'RET', l: 1 };
    t[0xCA] = { m: 'JP Z,a16', l: 3 };
    t[0xCB] = { m: 'PREFIX CB', l: 1 };
    t[0xCC] = { m: 'CALL Z,a16', l: 3 };
    t[0xCD] = { m: 'CALL a16', l: 3 };
    t[0xCE] = { m: 'ADC A,d8', l: 2 };
    t[0xCF] = { m: 'RST 08H', l: 1 };

    t[0xD0] = { m: 'RET NC', l: 1 };
    t[0xD1] = { m: 'POP DE', l: 1 };
    t[0xD2] = { m: 'JP NC,a16', l: 3 };
    t[0xD4] = { m: 'CALL NC,a16', l: 3 };
    t[0xD5] = { m: 'PUSH DE', l: 1 };
    t[0xD6] = { m: 'SUB d8', l: 2 };
    t[0xD7] = { m: 'RST 10H', l: 1 };
    t[0xD8] = { m: 'RET C', l: 1 };
    t[0xD9] = { m: 'RETI', l: 1 };
    t[0xDA] = { m: 'JP C,a16', l: 3 };
    t[0xDC] = { m: 'CALL C,a16', l: 3 };
    t[0xDE] = { m: 'SBC A,d8', l: 2 };
    t[0xDF] = { m: 'RST 18H', l: 1 };

    t[0xE0] = { m: 'LDH (a8),A', l: 2 };
    t[0xE1] = { m: 'POP HL', l: 1 };
    t[0xE2] = { m: 'LD (C),A', l: 1 };
    t[0xE5] = { m: 'PUSH HL', l: 1 };
    t[0xE6] = { m: 'AND d8', l: 2 };
    t[0xE7] = { m: 'RST 20H', l: 1 };
    t[0xE8] = { m: 'ADD SP,r8', l: 2 };
    t[0xE9] = { m: 'JP (HL)', l: 1 };
    t[0xEA] = { m: 'LD (a16),A', l: 3 };
    t[0xEE] = { m: 'XOR d8', l: 2 };
    t[0xEF] = { m: 'RST 28H', l: 1 };

    t[0xF0] = { m: 'LDH A,(a8)', l: 2 };
    t[0xF1] = { m: 'POP AF', l: 1 };
    t[0xF2] = { m: 'LD A,(C)', l: 1 };
    t[0xF3] = { m: 'DI', l: 1 };
    t[0xF5] = { m: 'PUSH AF', l: 1 };
    t[0xF6] = { m: 'OR d8', l: 2 };
    t[0xF7] = { m: 'RST 30H', l: 1 };
    t[0xF8] = { m: 'LD HL,SP+r8', l: 2 };
    t[0xF9] = { m: 'LD SP,HL', l: 1 };
    t[0xFA] = { m: 'LD A,(a16)', l: 3 };
    t[0xFB] = { m: 'EI', l: 1 };
    t[0xFE] = { m: 'CP d8', l: 2 };
    t[0xFF] = { m: 'RST 38H', l: 1 };

    return t;
}

function buildCBTable() {
    const t = new Array(256);
    const regs = ['B','C','D','E','H','L','(HL)','A'];
    const ops = ['RLC','RRC','RL','RR','SLA','SRA','SWAP','SRL'];

    for (let i = 0; i < 8; i++) {
        for (let r = 0; r < 8; r++) {
            t[i * 8 + r] = { m: `${ops[i]} ${regs[r]}` };
        }
    }
    for (let bit = 0; bit < 8; bit++) {
        for (let r = 0; r < 8; r++) {
            t[0x40 + bit * 8 + r] = { m: `BIT ${bit},${regs[r]}` };
            t[0x80 + bit * 8 + r] = { m: `RES ${bit},${regs[r]}` };
            t[0xC0 + bit * 8 + r] = { m: `SET ${bit},${regs[r]}` };
        }
    }
    return t;
}

/**
 * Disassemble `count` instructions starting at `pc`.
 * @param {(addr: number) => number} readByte - callback to read a byte
 * @param {number} pc - starting address
 * @param {number} count - number of instructions
 * @returns {{ addr: number, rawBytes: string, text: string, length: number }[]}
 */
export function disassemble(readByte, pc, count) {
    const lines = [];
    let addr = pc;
    for (let i = 0; i < count; i++) {
        const opcode = readByte(addr);
        let info, length, text, rawBytes;

        if (opcode === 0xCB) {
            const cb = readByte(addr + 1);
            info = CB_TABLE[cb];
            length = 2;
            rawBytes = `${hex2(opcode)} ${hex2(cb)}`;
            text = info.m;
        } else {
            info = OPCODE_TABLE[opcode];
            length = info.l;
            const bytes = [opcode];
            for (let b = 1; b < length; b++) bytes.push(readByte(addr + b));
            rawBytes = bytes.map(hex2).join(' ');
            text = info.m;

            if (length === 2) {
                const val = bytes[1];
                if (text.includes('r8')) {
                    const offset = val > 127 ? val - 256 : val;
                    const target = (addr + length + offset) & 0xFFFF;
                    text = text.replace('r8', `$${hex4(target)}`);
                } else if (text.includes('d8')) {
                    text = text.replace('d8', `$${hex2(val)}`);
                } else if (text.includes('a8')) {
                    text = text.replace('a8', `$FF${hex2(val)}`);
                }
            } else if (length === 3) {
                const val = bytes[1] | (bytes[2] << 8);
                if (text.includes('d16')) {
                    text = text.replace('d16', `$${hex4(val)}`);
                } else if (text.includes('a16')) {
                    text = text.replace('a16', `$${hex4(val)}`);
                }
            }
        }

        lines.push({ addr, rawBytes: rawBytes.padEnd(8), text, length });
        addr = (addr + length) & 0xFFFF;
    }
    return lines;
}
