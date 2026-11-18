#!/usr/bin/env python3
"""Exercise the signed shell installer entirely within disposable user homes."""
import hashlib
import os
from pathlib import Path
import plistlib
import subprocess
import tempfile


SOURCE = Path(__file__).resolve().parents[1] / "scripts/install-machine.sh"
ORIGIN = "https://fixture.invalid"
VERSION = "dev-fixture"
NAMESPACE = "calternal-machine-release-v1"


def write_executable(path, body):
    path.write_text(body)
    path.chmod(0o700)


def sign(key, manifest):
    subprocess.run(
        ["ssh-keygen", "-Y", "sign", "-n", NAMESPACE, "-f", key, manifest],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True,
    )


def setup(root):
    key = root / "development-only"
    subprocess.run(
        ["ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", key],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True,
    )
    public_key = " ".join(key.with_suffix(".pub").read_text().split()[:2])
    script = SOURCE.read_text().replace("release_key=\n", f"release_key='{public_key}'\n", 1)
    mirror = root / "releases"
    mirror.mkdir()
    (mirror / "version").write_text(VERSION + "\n")
    for platform in ("linux-x86_64", "macos-aarch64"):
        release = mirror / VERSION / platform
        release.mkdir(parents=True)
        binary = release / "calternal"
        write_executable(binary, """#!/bin/sh
set -eu
[ "$1" = machine ] && [ "$2" = enroll ]
[ "$3" = --server ] && [ "$4" = https://fixture.invalid ]
[ "$5" = --dir ] && [ "$7" = --code ] && [ "$8" = - ] && [ "$9" = --direct ]
[ -z "${CALTERNAL_ENROLL_CODE:-}" ]
IFS= read -r code
[ "$code" = disposable-code ]
mkdir -p "$6"
printf 'server: https://fixture.invalid\\n' > "$6/credentials"
printf 'enrolled\\n' >> "$HOME/trace"
""")
        content = binary.read_bytes()
        manifest = release / "manifest"
        manifest.write_text(f"{NAMESPACE}\n{VERSION}\n{platform}\n{len(content)}\n{hashlib.sha256(content).hexdigest()}\n")
        sign(key, manifest)

    tools = root / "tools"
    tools.mkdir()
    write_executable(tools / "curl", """#!/bin/sh
set -eu
out=
while [ "$#" -gt 1 ]; do
  if [ "$1" = --output ]; then out=$2; shift; fi
  shift
done
case $1 in
  https://fixture.invalid/api/v1/machine/release/*) version_path=${1#https://fixture.invalid/api/v1/machine/release/} ;;
  *) exit 1 ;;
esac
printf 'download %s\\n' "$version_path" >> "$HOME/trace"
cp "$RELEASE_FIXTURE/$version_path" "$out"
""")
    write_executable(tools / "uname", """#!/bin/sh
case $1 in
  -s) printf '%s\\n' "$FIXTURE_OS" ;;
  -m) printf '%s\\n' "$FIXTURE_ARCH" ;;
  *) exit 1 ;;
esac
""")
    write_executable(tools / "systemctl", """#!/bin/sh
[ "$1" = --user ] || exit 1
printf 'systemctl %s\\n' "$*" >> "$HOME/trace"
""")
    write_executable(tools / "launchctl", """#!/bin/sh
printf 'launchctl %s\\n' "$*" >> "$HOME/trace"
[ "$1" != print ]
""")
    return script, mirror, tools


def main():
    with tempfile.TemporaryDirectory(prefix="calternal-installer-") as workspace:
        root = Path(workspace)
        script, mirror, tools = setup(root)
        binary = mirror / VERSION / "linux-x86_64" / "calternal"
        signature = binary.with_name("manifest.sig")
        prefix = ".local/share/calternal/machine"

        def fixture(name, darwin=False):
            home = root / f"home {name}"
            home.mkdir()
            env = {
                "HOME": str(home), "PATH": f"{tools}:{os.environ['PATH']}",
                "RELEASE_FIXTURE": str(mirror), "CALTERNAL_ENROLL_CODE": "disposable-code",
                "FIXTURE_OS": "Darwin" if darwin else "Linux",
                "FIXTURE_ARCH": "arm64" if darwin else "x86_64",
            }
            def install(*flags, contents=script):
                return subprocess.run(
                    ["sh", "-s", "--", "--server", ORIGIN, "--backend", "direct", *flags],
                    input=contents, env=env, text=True, capture_output=True, timeout=15,
                )
            return home, install

        home, install = fixture("preview")
        result = install()
        assert result.returncode != 0 and "Download the machine program" in result.stdout
        assert not (home / "trace").exists()
        assert not (home / prefix).exists()
        result = install("--yes", contents=script.rsplit("\n}", 1)[0])
        assert result.returncode != 0 and not (home / "trace").exists()
        print("PASS preview and incomplete bootstrap leave the user home untouched")

        home, install = fixture("wrong signature")
        signed = signature.read_bytes()
        signature.write_bytes(b"invalid signature")
        result = install("--yes")
        assert result.returncode != 0 and "signature is invalid" in result.stderr
        assert "/calternal" not in (home / "trace").read_text()
        assert not (home / prefix).exists()
        signature.write_bytes(signed)
        print("PASS invalid signature stops before downloading or executing the program")

        home, install = fixture("wrong digest")
        payload = binary.read_bytes()
        binary.write_bytes(b"x" + payload[1:])
        result = install("--yes")
        assert result.returncode != 0 and "digest does not match" in result.stderr
        assert not (home / prefix).exists()
        binary.write_bytes(payload)
        print("PASS altered executable cannot run")

        home, install = fixture("linux")
        result = install("--yes")
        assert result.returncode == 0, result.stderr
        unit = home / ".config/systemd/user/calternal-machine.service"
        target = home / prefix / "bin/calternal"
        assert target.read_bytes() == payload
        assert "--self-update" in unit.read_text()
        trace = (home / "trace").read_text()
        assert trace.count("enrolled") == 1
        assert "systemctl --user restart calternal-machine.service" in trace
        again = install("--yes")
        assert again.returncode == 0, again.stderr
        assert (home / "trace").read_text().count("enrolled") == 1
        result = install("--yes", "--uninstall")
        assert result.returncode == 0, result.stderr
        assert not target.exists() and not unit.exists()
        assert (home / prefix / "credentials").exists()
        assert "systemctl --user disable --now" in (home / "trace").read_text()
        print("PASS Linux user install, enrollment, repeat install and uninstall")

        home, install = fixture("darwin", darwin=True)
        result = install("--yes")
        assert result.returncode == 0, result.stderr
        agent = home / "Library/LaunchAgents/org.calternal.machine.plist"
        contents = plistlib.loads(agent.read_bytes())
        assert contents["ProgramArguments"][0] == str(home / prefix / "bin/calternal")
        assert "--self-update" in contents["ProgramArguments"]
        assert "launchctl bootstrap" in (home / "trace").read_text()
        result = install("--yes", "--uninstall")
        assert result.returncode == 0, result.stderr
        assert not agent.exists()
        print("PASS simulated Darwin LaunchAgent generation and removal")


if __name__ == "__main__":
    main()
