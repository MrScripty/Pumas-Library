const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const root = path.resolve(__dirname, '../..');
function checkAttribution() {
  const version = JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version;
  const directory = path.join(root, 'docs/release-attribution', version);
  const inventory = JSON.parse(fs.readFileSync(path.join(directory, 'inventory.json')));
  const hash = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  for (const file of ['rust/Cargo.lock', 'pnpm-lock.yaml', 'electron/package.json', 'frontend/package.json']) {
    if (hash(path.join(root, file)) !== inventory.input_sha256[file]) throw new Error(`Stale release attribution: ${file}`);
  }
  if (hash(path.join(directory, 'THIRD-PARTY-NOTICES.txt')) !== inventory.notices_sha256) throw new Error('Changed release notices');
  if (!inventory.packages?.length) throw new Error('Empty release attribution inventory');
  for (const source of inventory.sources) {
    if (hash(path.join(root, 'scripts/release/licenses', source.file)) !== source.sha256) throw new Error(`Changed upstream license: ${source.file}`);
  }
}
module.exports = checkAttribution;
if (require.main === module) {
  checkAttribution();
  console.log('Release attribution matches current inputs and pinned texts');
}
