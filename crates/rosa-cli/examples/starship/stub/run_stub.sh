#!/usr/bin/env bash
# run_stub.sh — Start the Starship physics stub inside rosa-ros2.
#
# Use this when Isaac Sim is not available (faster CI demos, offline runs).
# The stub publishes the same /starship/* ROS 2 topic contract as Isaac Sim
# so the rosa agent works identically against either backend.
#
# Usage (from repo root):
#   ./crates/rosa-cli/examples/starship/stub/run_stub.sh
#
# Or with a custom container name:
#   ROS_CONTAINER=my-ros2-container ./run_stub.sh
#
# Requires:
#   - rosa-ros2 container running (docker compose up -d in Monoclaw/Deployments/Stacks/ROS/)
#   - /tmp/starship_stub/ writable (or set STUB_DIR)

set -e

CONTAINER="${ROS_CONTAINER:-rosa-ros2}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
STUB_SCRIPT="$SCRIPT_DIR/starship_sim_node.py"

if ! docker inspect "$CONTAINER" &>/dev/null; then
    echo "ERROR: container '$CONTAINER' is not running."
    echo "  Start it: cd Monoclaw/Deployments/Stacks/ROS && docker compose up -d"
    exit 1
fi

echo "=== Starship simulation stub ==="
echo "  container : $CONTAINER"
echo "  script    : $STUB_SCRIPT"
echo ""

# Copy the stub script into the container at /tmp/starship_sim_node.py
docker cp "$STUB_SCRIPT" "$CONTAINER":/tmp/starship_sim_node.py

echo "Starting stub (Ctrl-C to stop) ..."
docker exec -it "$CONTAINER" bash -ic \
    "cd /tmp && python3 /tmp/starship_sim_node.py"
