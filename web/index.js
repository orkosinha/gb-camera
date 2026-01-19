import init, { greet } from './pkg/gb_emu.js';

init().then(() => {
    greet('WebAssembly');
});
