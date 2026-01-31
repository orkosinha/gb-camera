// ── Save / Load .sav files ──────────────────────────────────────────

export function createSave(state) {
    function download() {
        if (!state.emulator) return;
        const saveData = state.emulator.get_cartridge_ram();
        const blob = new Blob([saveData], { type: 'application/octet-stream' });
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `${state.currentRomName}.sav`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
    }

    async function load(file) {
        if (!state.emulator) return;
        const buffer = await file.arrayBuffer();
        state.emulator.load_cartridge_ram(new Uint8Array(buffer));
    }

    return { download, load };
}
