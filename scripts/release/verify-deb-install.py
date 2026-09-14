#!/usr/bin/env python3
"""Install/remove a trusted Debian candidate in an isolated Linux namespace.

Usage: verify-deb-install.py /path/to/candidate.deb
Requires bubblewrap, the host Debian toolchain and about 1 GB temporary space.
The printed temporary directory retains the isolated verification state.
"""

import subprocess
import tempfile
import shutil
import sys
from pathlib import Path

root = Path(tempfile.mkdtemp(prefix="pumas-deb-install-"))
package = Path(sys.argv[1]).resolve()
for name, source in [
    ("bin", "/usr/bin"),
    ("dpkg", "/var/lib/dpkg"),
    ("applications", "/usr/share/applications"),
    ("mime", "/usr/share/mime"),
]:
    if (root / name).exists():
        shutil.rmtree(root / name)
    shutil.copytree(
        source,
        root / name,
        symlinks=True,
        ignore=shutil.ignore_patterns("lock", "lock-frontend", "Lock"),
    )
if (root / "etc").exists():
    shutil.rmtree(root / "etc")
(root / "etc").mkdir()
for name in [
    "alternatives",
    "dpkg",
    "ld.so.cache",
    "mailcap",
    "mime.types",
    "os-release",
    "passwd",
    "group",
    "nsswitch.conf",
]:
    source = Path("/etc") / name
    if source.is_dir():
        shutil.copytree(source, root / "etc" / name, symlinks=True)
    elif source.exists():
        shutil.copyfile(source, root / "etc" / name)
(root / "etc/apparmor.d").mkdir()
(root / "icons/hicolor").mkdir(parents=True, exist_ok=True)
shutil.copyfile("/usr/share/icons/hicolor/index.theme", root / "icons/hicolor/index.theme")
for name in ["opt", "log", "cache", "tmp"]:
    (root / name).mkdir(exist_ok=True)
script = root / "verify.sh"
script.write_text("""set -eu
dpkg --install /tmp/candidate.deb
dpkg-query -W -f='${Status} ${Version}\\n' pumas-library-electron
test "$(readlink /etc/alternatives/pumas-library-electron)" = '/opt/Pumas Library/pumas-library-electron'
test -s '/opt/Pumas Library/resources/THIRD-PARTY-NOTICES.txt'
ELECTRON_RUN_AS_NODE=1 /usr/bin/pumas-library-electron -p process.versions.electron
dpkg --remove pumas-library-electron
test ! -e /etc/alternatives/pumas-library-electron
test ! -e '/opt/Pumas Library/pumas-library-electron'
echo 'Isolated Debian installation, alternatives, runtime identity and removal passed'
""")
subprocess.run(["cp", str(package), str(root / "tmp/candidate.deb")], check=True)
args = [
    "bwrap",
    "--unshare-user",
    "--uid",
    "0",
    "--gid",
    "0",
    "--unshare-pid",
    "--unshare-net",
    "--ro-bind",
    "/",
    "/",
    "--proc",
    "/proc",
    "--dev",
    "/dev",
]
for name, target in [
    ("bin", "/usr/bin"),
    ("etc", "/etc"),
    ("icons", "/usr/share/icons"),
    ("dpkg", "/var/lib/dpkg"),
    ("applications", "/usr/share/applications"),
    ("mime", "/usr/share/mime"),
    ("opt", "/opt"),
    ("log", "/var/log"),
    ("cache", "/var/cache"),
    ("tmp", "/tmp"),
]:
    args += ["--bind", str(root / name), target]
args += [
    "--tmpfs",
    "/usr/share/doc",
    "--ro-bind",
    str(script),
    "/tmp/verify.sh",
    "/bin/bash",
    "/tmp/verify.sh",
]
print("Isolated root:", root, flush=True)
subprocess.run(args, check=True)
