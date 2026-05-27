#!/usr/bin/env bash
# run_demo.sh — One-shot setup for the Starship ROSA demo
# Usage: ./run_demo.sh
set -e

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROS_STACK_DIR="/home/propdev/.openclaw/workspace/repos/Monoclaw/Deployments/Stacks/ROS"
STUB="$REPO_DIR/crates/rosa-cli/examples/starship/stub/starship_sim_node.py"
CONTAINER="${ROS_CONTAINER:-rosa-ros2}"

echo "════════════════════════════════════════════════════════════"
echo "  ROSA Starship Demo — Mars Proximity Ops"
echo "  Model: grok-4.3 via xAI API"
echo "════════════════════════════════════════════════════════════"
echo ""

# ── 1. Ensure ROS 2 container is running ─────────────────────────────────────
if ! docker inspect "$CONTAINER" &>/dev/null 2>&1; then
    echo "→ Starting ROS 2 container..."
    (cd "$ROS_STACK_DIR" && docker compose up -d) || {
        echo "ERROR: could not start ROS container."
        echo "  cd $ROS_STACK_DIR && docker compose up -d"
        exit 1
    }
    sleep 3
fi
echo "✓ ROS 2 container: $CONTAINER"

# ── 2. Copy & start the physics stub ─────────────────────────────────────────
echo "→ Starting Starship physics stub (Mars, 1000 m AGL)..."
docker cp "$STUB" "$CONTAINER":/tmp/starship_sim_node.py
# Kill any running stub — use "python3.*starship_sim" to avoid matching this bash invocation.
docker exec "$CONTAINER" bash -c 'kill $(pgrep -f "python3.*starship_sim") 2>/dev/null; sleep 0.3; true' || true
sleep 1
# Start stub in background — must source ROS 2 setup so rclpy is importable.
docker exec -d "$CONTAINER" bash -c "source /opt/ros/humble/setup.bash && python3 /tmp/starship_sim_node.py"
sleep 5

# ── 3. Pre-apply hover throttle so ship is hovering when demo starts ──────────
# At full fuel (830 t total): T_hover = 830000 * 3.72 / 14700000 ≈ 0.210
echo "→ Pre-applying hover throttle 0.210 (full-fuel hover at Mars g)..."
docker exec "$CONTAINER" bash -c "
  source /opt/ros/humble/setup.bash 2>/dev/null
  ros2 topic pub --once /starship/main_throttle std_msgs/msg/Float32 '{data: 0.210}' 2>/dev/null
  echo 'Throttle 0.210 applied'
" 2>/dev/null || echo "  (throttle pre-apply skipped)"
sleep 2

# ── 4. Verify topics and altitude ────────────────────────────────────────────
TOPICS=$(docker exec "$CONTAINER" bash -c \
    "source /opt/ros/humble/setup.bash && ros2 topic list 2>/dev/null | grep starship | wc -l" 2>/dev/null || echo "0")
ALT=$(docker exec "$CONTAINER" bash -c \
    "source /opt/ros/humble/setup.bash && timeout 2 ros2 topic echo /starship/altitude std_msgs/msg/Float64 --once 2>/dev/null | grep data | head -1" 2>/dev/null || echo "data: ?")
ENGINE=$(docker exec "$CONTAINER" bash -c \
    "source /opt/ros/humble/setup.bash && timeout 2 ros2 topic echo /starship/engine_state std_msgs/msg/String --once 2>/dev/null | head -2" 2>/dev/null || echo "?")

echo "✓ $TOPICS starship topics live"
echo "✓ Current altitude: $ALT"
echo "✓ Engine state: $ENGINE"

echo ""
echo "════════════════════════════════════════════════════════════"
echo "  Starship is HOVERING near 1000 m AGL (Mars)"
echo ""
echo "  Suggested demo prompts:"
echo "  1. 'What is the current altitude, fuel, and engine state?'"
echo "  2. 'Perform a controlled descent to 500 m'"
echo "  3. 'Execute a 5-degree pitch gimbal burn'"
echo "  4. 'ABORT! Emergency abort!'"
echo "  5. 'Fuel is at 3%%. What do you do?'"
echo ""
echo "  Type /quit to exit ROSA"
echo "════════════════════════════════════════════════════════════"
echo ""

# ── 5. Launch ROSA ────────────────────────────────────────────────────────────
cd "$REPO_DIR"
exec env ROS_CONTAINER="$CONTAINER" cargo run --example starship -p rosa-cli
