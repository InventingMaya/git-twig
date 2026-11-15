#!/usr/bin/env python3
"""Fail if a release tag does not match the workspace version."""
import pathlib
import sys
import tomllib

version = tomllib.loads(pathlib.Path("Cargo.toml").read_text())["workspace"]["package"]["version"]
if sys.argv[1] != version:
    raise SystemExit(f"tag version {sys.argv[1]} differs from Cargo workspace {version}")
