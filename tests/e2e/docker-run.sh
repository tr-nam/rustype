#!/usr/bin/env bash
# GNOME 46 target: builds tests/e2e/Dockerfile from a tar of the working tree, then runs the suite inside a
# container with --network none. Only rustype-e2e* images and containers are created, labelled rustype-e2e=1.
# No --privileged, no bind mounts, no docker socket. Cleanup removes only this run's container and image.
set -u
REPO=$(cd "$(dirname "$0")/../.." && pwd)
TAG=rustype-e2e:gnome46
NAME=rustype-e2e-run-$$
cleanup() {
    docker rm -f "$NAME" >/dev/null 2>&1 || true
}
trap cleanup EXIT
# Build context: the working tree without target/ and .git, streamed as a tar.
( cd "$REPO" && tar --exclude=./target --exclude=./.git -cf - . ) \
  | docker build --label rustype-e2e=1 -t "$TAG" -f tests/e2e/Dockerfile - || { echo "docker build failed" >&2; exit 4; }
# The test run itself: no network, no privileges, no host mounts.
docker run --rm --name "$NAME" --label rustype-e2e=1 --network none \
    --security-opt no-new-privileges --cap-drop ALL --pids-limit 512 --memory 4g \
    "$TAG" "$@"
