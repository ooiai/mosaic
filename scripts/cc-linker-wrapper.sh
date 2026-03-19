#!/bin/sh

# Some local GCC setups try to read ./specs from the current working directory.
# Run the real linker from /tmp so the workspace's specs/ directory is not
# misinterpreted as a GCC specs file during Cargo link steps.
cd /tmp || exit 1
exec "${REAL_CC_BIN:-/usr/bin/cc}" "$@"
