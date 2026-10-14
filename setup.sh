#!/bin/sh
#
# Setup script for sass-dep development.
# Run this once after cloning the repository.

set -e

echo "Setting up sass-dep development environment..."

# Use the project's git hooks.
git config core.hooksPath .githooks
echo "Git hooks enabled"

# Install and build the web visualizer so `cargo build` can embed it.
if command -v npm >/dev/null 2>&1 && [ -d "web" ]; then
    echo "Installing and building the web visualizer..."
    (cd web && npm ci && npm run build)
else
    echo "Warning: npm not found; the CLI will build without the embedded visualizer"
fi

echo "Setup complete! Run 'cargo build' or 'cargo install --path .'"
