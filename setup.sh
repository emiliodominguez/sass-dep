#!/bin/sh
#
# Setup script for sass-dep development
# Run this once after cloning the repository

set -e

echo "Setting up sass-dep development environment..."

# Configure git to use the project's hooks
git config core.hooksPath .githooks
echo "Git hooks enabled"

# Install web dependencies if npm is available
if command -v npm &> /dev/null && [ -d "web" ]; then
    echo "Installing web dependencies..."
    cd web && npm install
    cd ..
fi

echo "Setup complete!"
