// ── Keyboard handling ───────────────────────────────────────────────

// Button indices must match joypad::Button::from_u8 in Rust:
//   0=A, 1=B, 2=Select, 3=Start, 4=Right, 5=Left, 6=Up, 7=Down
const BUTTON_MAP = {
    'z': 0, 'Z': 0,       // A
    'x': 1, 'X': 1,       // B
    Shift: 2,              // Select
    Enter: 3,              // Start
    ArrowRight: 4,
    ArrowLeft: 5,
    ArrowUp: 6,
    ArrowDown: 7,
};

const BUTTON_NAMES = {
    0: 'a', 1: 'b', 2: 'select', 3: 'start',
    4: 'right', 5: 'left', 6: 'up', 7: 'down',
};

export function createInput(state, buttonState) {
    function handleKeyDown(e) {
        if (e.target.tagName !== 'INPUT') {
            if (e.code === 'Space') {
                e.preventDefault();
                state.paused = !state.paused;
                if (!state.paused) { state.panelUpdateCounter = 0; }
                return;
            }
            if (e.key === 'n' || e.key === 'N') {
                if (state.paused && state.emulator) { state.stepMode = 'instruction'; }
                return;
            }
            if (e.key === 'f' && !e.ctrlKey && !e.metaKey) {
                if (state.paused && state.emulator) { state.stepMode = 'frame'; }
                return;
            }
        }

        if (!state.emulator || state.paused) return;
        const btn = BUTTON_MAP[e.key];
        if (btn !== undefined) {
            e.preventDefault();
            state.emulator.set_button(btn, true);
            buttonState[BUTTON_NAMES[btn]] = true;
        }
    }

    function handleKeyUp(e) {
        if (!state.emulator) return;
        const btn = BUTTON_MAP[e.key];
        if (btn !== undefined) {
            state.emulator.set_button(btn, false);
            buttonState[BUTTON_NAMES[btn]] = false;
        }
    }

    function attach() {
        window.addEventListener('keydown', handleKeyDown);
        window.addEventListener('keyup', handleKeyUp);
    }

    function detach() {
        window.removeEventListener('keydown', handleKeyDown);
        window.removeEventListener('keyup', handleKeyUp);
    }

    return { attach, detach };
}
