#!/usr/bin/env bash

# Script to run Neovim plugin tests for snapper

set -e

# Check if Neovim is installed
if ! command -v nvim &> /dev/null; then
    echo "Error: Neovim is not installed"
    exit 1
fi

# Check if snapper binary is available
if ! command -v snapper &> /dev/null; then
    echo "Building snapper binary..."
    cargo build --release
    export PATH="$PWD/target/release:$PATH"
fi

NVIM_DATA="${XDG_DATA_HOME:-$HOME/.local/share}/nvim"

# Install plenary.nvim if not present
if [ ! -d "$NVIM_DATA/site/pack/test/start/plenary.nvim" ]; then
    echo "Installing plenary.nvim for testing..."
    mkdir -p "$NVIM_DATA/site/pack/test/start"
    git clone --depth 1 https://github.com/nvim-lua/plenary.nvim \
        "$NVIM_DATA/site/pack/test/start/plenary.nvim"
fi

# Run the tests
echo "Running Neovim plugin tests..."
nvim --headless -u editors/nvim/tests/minimal_init.lua \
    -c "lua require('plenary.test_harness').test_directory('editors/nvim/tests/', {minimal_rc = 'editors/nvim/tests/minimal_init.lua'})" \
    -c "qa"

echo "Neovim tests completed!"
