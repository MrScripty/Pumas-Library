// No-inference desktop packaging configuration.
//
// Reuses the base electron-builder configuration from package.json and swaps
// in the distinct application identity and installer names for the
// GUI-without-inference line, which embeds the --no-default-features backend
// staged in resources/bin. Invoked as:
//   electron-builder --linux|--mac|--win --publish never -c ./electron-builder.no-inference.cjs
// (a config file path: electron-builder has no `-c.key=value` CLI overrides,
// so flags like `-c.appId=...` are misread as a config filename).
const base = require('./package.json').build;

module.exports = {
  ...base,
  appId: 'com.pumas.library.no-inference',
  appImage: {
    ...base.appImage,
    artifactName: 'Pumas.Library-no-inference-${version}.${ext}',
  },
  deb: {
    ...base.deb,
    artifactName: 'pumas-library-electron-no-inference_${version}_amd64.${ext}',
  },
  nsis: {
    ...base.nsis,
    artifactName: 'Pumas.Library.Setup.no-inference.${version}.${ext}',
  },
  portable: {
    ...base.portable,
    artifactName: 'Pumas.Library.no-inference.${version}.${ext}',
  },
  dmg: {
    ...base.dmg,
    artifactName: 'Pumas.Library-no-inference-${version}-arm64.${ext}',
  },
};
