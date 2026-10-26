#!/usr/bin/env python3
"""Verify the imported contract baseline, then check native code when present."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]

def main():
    manifest = json.loads((ROOT / "docs/SOURCE_MANIFEST.json").read_text(encoding="utf-8"))
    for name, expected in manifest.items():
        path = ROOT / "replica-spec" / name
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != expected:
            raise RuntimeError(f"Imported specification baseline changed: {name}")
    if (ROOT / "docs/SPEC.md").read_bytes() != (ROOT / "replica-spec/DEVELOPMENT_SPEC.md").read_bytes():
        raise RuntimeError("SPEC differs from imported DEVELOPMENT_SPEC")
    print(f"Imported specification integrity: {len(manifest)} files", flush=True)
    cargo = ROOT / "src-tauri/Cargo.toml"
    if not cargo.exists():
        print("Native fmt/check/clippy/test: NOT RUN; src-tauri is planned in T01/T05", flush=True)
        return 0
    commands = [
        ["cargo", "fmt", "--manifest-path", str(cargo), "--", "--check"],
        ["cargo", "check", "--locked", "--manifest-path", str(cargo)],
        ["cargo", "clippy", "--locked", "--manifest-path", str(cargo), "--all-targets", "--", "-D", "warnings"],
        ["cargo", "test", "--locked", "--manifest-path", str(cargo), "--", "--test-threads=1"],
    ]
    for command in commands:
        print(json.dumps(command), flush=True)
        subprocess.run(command, cwd=ROOT, check=True, timeout=1800)
    return 0

if __name__ == "__main__":
    sys.exit(main())
