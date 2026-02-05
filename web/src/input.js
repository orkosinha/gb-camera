// ── Input handling (keyboard + touch) ───────────────────────────────

const BUTTONS = {
    'z': 0, 'Z': 0,       // A
    'x': 1, 'X': 1,       // B
    Shift: 2,              // Select
    Enter: 3,              // Start
    ArrowRight: 4,
    ArrowLeft: 5,
    ArrowUp: 6,
    ArrowDown: 7,
};

const NAMES = ['a', 'b', 'select', 'start', 'right', 'left', 'up', 'down'];

export function createInput(state, buttonState) {
    // Track active touches per button
    const activeTouches = new Map();

    // Get gamepad button element by button index
    function getButtonElement(btn) {
        return document.querySelector(`.gp-btn[data-btn="${btn}"]`);
    }

    function pressButton(btn, fromKeyboard = false) {
        if (!state.emulator || state.paused) return;
        state.emulator.set_button(btn, true);
        buttonState[NAMES[btn]] = true;
        if (fromKeyboard) {
            const el = getButtonElement(btn);
            if (el) el.classList.add('pressed');
        }
    }

    function releaseButton(btn, fromKeyboard = false) {
        if (!state.emulator) return;
        state.emulator.set_button(btn, false);
        buttonState[NAMES[btn]] = false;
        if (fromKeyboard) {
            const el = getButtonElement(btn);
            if (el) el.classList.remove('pressed');
        }
    }

    // Keyboard handlers
    function onKeyDown(e) {
        if (e.target.tagName === 'INPUT') return;

        if (e.code === 'Space') {
            e.preventDefault();
            state.paused = !state.paused;
            return;
        }
        if (e.key === 'n' || e.key === 'N') {
            if (state.paused && state.emulator) state.stepMode = 'instruction';
            return;
        }
        if (e.key === 'f' && !e.ctrlKey && !e.metaKey) {
            if (state.paused && state.emulator) state.stepMode = 'frame';
            return;
        }

        const btn = BUTTONS[e.key];
        if (btn !== undefined) {
            e.preventDefault();
            pressButton(btn, true);
        }
    }

    function onKeyUp(e) {
        const btn = BUTTONS[e.key];
        if (btn !== undefined) {
            releaseButton(btn, true);
        }
    }

    // Touch handlers for gamepad
    function onTouchStart(e) {
        const target = e.target.closest('.gp-btn');
        if (!target) return;
        e.preventDefault();

        const btn = parseInt(target.dataset.btn, 10);
        if (isNaN(btn)) return;

        for (const touch of e.changedTouches) {
            activeTouches.set(touch.identifier, { btn, target });
        }
        target.classList.add('pressed');
        pressButton(btn);
    }

    function onTouchEnd(e) {
        for (const touch of e.changedTouches) {
            const info = activeTouches.get(touch.identifier);
            if (info) {
                info.target.classList.remove('pressed');
                releaseButton(info.btn);
                activeTouches.delete(touch.identifier);
            }
        }
    }

    function onTouchCancel(e) {
        onTouchEnd(e);
    }

    // Mouse handlers for desktop testing
    function onMouseDown(e) {
        const target = e.target.closest('.gp-btn');
        if (!target) return;
        e.preventDefault();

        const btn = parseInt(target.dataset.btn, 10);
        if (isNaN(btn)) return;

        target.classList.add('pressed');
        pressButton(btn);
        target.dataset.mouseDown = '1';
    }

    function onMouseUp(e) {
        const target = e.target.closest('.gp-btn');
        if (!target || !target.dataset.mouseDown) return;

        const btn = parseInt(target.dataset.btn, 10);
        if (isNaN(btn)) return;

        target.classList.remove('pressed');
        releaseButton(btn);
        delete target.dataset.mouseDown;
    }

    function onMouseLeave(e) {
        const target = e.target.closest('.gp-btn');
        if (!target || !target.dataset.mouseDown) return;

        const btn = parseInt(target.dataset.btn, 10);
        if (!isNaN(btn)) {
            target.classList.remove('pressed');
            releaseButton(btn);
        }
        delete target.dataset.mouseDown;
    }

    return {
        attach() {
            window.addEventListener('keydown', onKeyDown);
            window.addEventListener('keyup', onKeyUp);

            const gamepad = document.getElementById('gamepad');
            if (gamepad) {
                gamepad.addEventListener('touchstart', onTouchStart, { passive: false });
                gamepad.addEventListener('touchend', onTouchEnd);
                gamepad.addEventListener('touchcancel', onTouchCancel);
                gamepad.addEventListener('mousedown', onMouseDown);
                gamepad.addEventListener('mouseup', onMouseUp);
                gamepad.addEventListener('mouseleave', onMouseLeave, true);
            }
        },
        detach() {
            window.removeEventListener('keydown', onKeyDown);
            window.removeEventListener('keyup', onKeyUp);

            const gamepad = document.getElementById('gamepad');
            if (gamepad) {
                gamepad.removeEventListener('touchstart', onTouchStart);
                gamepad.removeEventListener('touchend', onTouchEnd);
                gamepad.removeEventListener('touchcancel', onTouchCancel);
                gamepad.removeEventListener('mousedown', onMouseDown);
                gamepad.removeEventListener('mouseup', onMouseUp);
                gamepad.removeEventListener('mouseleave', onMouseLeave, true);
            }
        }
    };
}
