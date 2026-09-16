// Keep the hook entry point inside the Electron project when the packager
// detects this directory as its workspace root (including native Windows).
module.exports = require('../../scripts/release/check-attribution.cjs');
