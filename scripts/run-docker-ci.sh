#!/usr/bin/env bash
# Run the full Linux CI pipeline inside Docker.
# This mirrors the GitHub Actions environment and catches environment-specific
# failures before they reach CI.
set -euo pipefail

cd "$(dirname "$0")/.."

export DOCKER_BUILDKIT=1

echo "Building klyntbot:ci Docker image..."
docker build -f docker/Dockerfile.ci -t klyntbot:ci "$@" .

echo "Docker CI build succeeded."
