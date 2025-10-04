const fs = require('node:fs');
const path = require('node:path');

const projectRoot = path.resolve(__dirname, '..');
const appDirs = [
  path.join(projectRoot, 'src-tauri', 'target', 'release', 'bundle', 'appimage', 'pastrami.AppDir'),
  path.join(projectRoot, 'src-tauri', 'target', 'debug', 'bundle', 'appimage', 'pastrami.AppDir'),
];

for (const dir of appDirs) {
  try {
    fs.rmSync(dir, { recursive: true, force: true });
    console.log(`Removed stale AppImage staging directory: ${dir}`);
  } catch (error) {
    console.warn(`Unable to remove AppImage staging directory ${dir}: ${error.message}`);
  }
}
