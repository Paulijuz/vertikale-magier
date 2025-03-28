#!/bin/bash

if [ -z "$1" ] || [ -z "$2" ]; then
    echo "Usage: $0 <name> <port> [--no-backup]"
    exit 1
fi

NAME="$1"
PORT="$2"
NO_BACKUP="$3"

while true; do
    echo "Starting cargo run with name $NAME on port $PORT..."
    if [ "$NO_BACKUP" == "--no-backup" ]; then
        cargo run -- --name "$NAME" --port "$PORT" --no-backup
    else
        cargo run -- --name "$NAME" --port "$PORT"
    fi

    echo "Process crashed. Restarting in 1 second..."
    sleep 1
done
