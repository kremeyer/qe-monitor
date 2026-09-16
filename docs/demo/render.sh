#!/usr/bin/env bash
# Regenerate docs/demo.gif.
# Needs: CC=gcc cargo install --git https://github.com/asciinema/agg --locked
cd "$(dirname "$0")" || exit 1
cargo build --release
python3 record.py demo.cast
agg --font-size 14 --theme monokai --last-frame-duration 3 demo.cast ../demo.gif
