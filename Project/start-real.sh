#!/bin/bash

# Find an available port starting from 8000
PORT=8000

while ss -tuln | grep -q ":$PORT"; do
    ((PORT++))
done

echo "Starting elevatorserver on port $PORT..."
elevatorserverer --port "$PORT"