#!/usr/bin/env python3
"""Build the local fork and Code Mode host with verified, matching V8 artifacts."""

import argparse
import os
from pathlib import Path
import subprocess
import sys


REPO_ROOT = Path(__file__).resolve().parent.parent
os.environ["CODEX_REPO_ROOT"] = str(REPO_ROOT)
sys.path.insert(0, str(REPO_ROOT / "scripts"))

from codex_package.targets import TARGET_SPECS
from codex_package.v8 import resolve_codex_v8_cargo_env


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--release", action="store_true")
    args = parser.parse_args()
    rust_dir = REPO_ROOT / "codex-rs"
    rust_version = subprocess.check_output(["rustc", "-vV"], cwd=rust_dir, text=True)
    target = next(
        line.removeprefix("host: ")
        for line in rust_version.splitlines()
        if line.startswith("host: ")
    )
    build_env = os.environ.copy()
    build_env.update(
        resolve_codex_v8_cargo_env(
            TARGET_SPECS[target], cache_root=rust_dir / "target" / "manual-v8"
        )
    )
    command = ["cargo", "build", "--bin", "codex", "--bin", "codex-code-mode-host"]
    if args.release:
        command.append("--release")
    return subprocess.call(command, cwd=rust_dir, env=build_env)


if __name__ == "__main__":
    raise SystemExit(main())
