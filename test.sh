#!/bin/bash
# A handy wrapper for testing things.

cargo run -- -f test.log -l trace -m overwrite $@
