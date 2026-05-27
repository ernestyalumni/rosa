#!/usr/bin/env bash
# run_demo.sh — Launch the Starship FSW demo
# 
# Prerequisites:
#   - rosa-ros2 container running with stub: ./crates/rosa-cli/examples/starship/stub/run_stub.sh
#   - ANTHROPIC_API_KEY set here or in .env
#
# Usage:
#   ANTHROPIC_API_KEY=sk-ant-... ./run_demo.sh
#   # or create .env with the key and just: ./run_demo.sh

set -e
cd "$(dirname "$0")"

# Load .env if it exists
[ -f .env ] && source .env

if [ -z "$ANTHROPIC_API_KEY" ] && [ -z "$XAI_API_KEY" ] && [ -z "$OPENAI_API_KEY" ]; then
    echo "ERROR: Set ANTHROPIC_API_KEY (or XAI_API_KEY / OPENAI_API_KEY) in .env or environment"
    exit 1
fi

echo "=== Starship FSW Demo ==="
echo "stub: running in rosa-ros2 container"
echo "container: ${ROS_CONTAINER:-rosa-ros2}"
echo ""

ROS_CONTAINER="${ROS_CONTAINER:-rosa-ros2}" \
    cargo run --release --example starship -p rosa-cli
