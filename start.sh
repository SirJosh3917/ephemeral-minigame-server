#!/bin/sh
set -e

# Build our plugins
#
# This is... a very barbaric way to use Docker.
# Ideally we'd use Bazel, but I don't feel like figuring that out :p
(cd minigameplugin && docker build . -t minigame-plugin)
(cd serverplugin && docker build . -t server-plugin)

# Build our server images (these will be spawned by Docker daemons)
(cd servers && docker build . -f Dockerfile.proxy -t ems-proxy)
(cd servers && docker build . -f Dockerfile.lobby -t ems-lobby)
(cd servers && docker build . -f Dockerfile.minigame -t ems-minigame)

# Allow for `./start.sh -d`
docker compose up --build "$@"
