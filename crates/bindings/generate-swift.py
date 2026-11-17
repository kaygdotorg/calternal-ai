#!/usr/bin/env python3
"""Generate matching Swift/C artifacts and refresh the SwiftPM library's bridge."""
from pathlib import Path
import shutil
import subprocess
import sys

root = Path(__file__).resolve().parents[2]
library = Path(sys.argv[1]) if len(sys.argv) > 1 else root / "target/debug/libcalternal_bindings.dylib"
output = root / "crates/bindings/generated/swift"
subprocess.run([str(root / "target/debug/calternal-bindgen"), "generate", "--library", str(library), "--language", "swift", "--out-dir", str(output)], check=True)
for path in output.iterdir():
    if path.is_file():
        path.write_text("\n".join(line.rstrip() for line in path.read_text().splitlines()).rstrip() + "\n")
package = root / "apps/apple/CalternalCore/Sources"
shutil.copyfile(output / "calternal_bindings.swift", package / "CalternalCore/calternal_bindings.swift")
shutil.copyfile(output / "calternal_bindingsFFI.h", package / "calternal_bindingsFFI/include/calternal_bindingsFFI.h")
shutil.copyfile(output / "calternal_bindingsFFI.modulemap", package / "calternal_bindingsFFI/include/module.modulemap")
