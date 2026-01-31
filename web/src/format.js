// ── Pure formatting helpers ──────────────────────────────────────────

export function hex2(v) {
    return v.toString(16).toUpperCase().padStart(2, '0');
}

export function hex4(v) {
    return v.toString(16).toUpperCase().padStart(4, '0');
}

export function escapeHtml(str) {
    return str.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
