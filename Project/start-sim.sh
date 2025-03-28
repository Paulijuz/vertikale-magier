#!/bin/bash

# Find an available port starting from 8000
PORT=8000

while ss -tuln | grep -q ":$PORT"; do
    ((PORT++))
done

echo "Starting simelevatorserver on port $PORT..."
simelevatorserver --port "$PORT"