#!/usr/bin/env bash
set -e

cargo run -- -f test.log -m overwrite $@
