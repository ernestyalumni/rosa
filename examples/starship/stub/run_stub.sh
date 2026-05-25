#!/usr/bin/env bash
# run_stub.sh — Publish fake Starship telemetry for demo use without Isaac Sim.
#
# Publishes all /starship/* topics that rosa-starship reads:
#   /starship/altitude       std_msgs/msg/Float64   (slowly increasing)
#   /starship/fuel_fraction  std_msgs/msg/Float32   (slowly decreasing)
#   /starship/engine_state   std_msgs/msg/String    (JSON status)
#   /starship/velocity       geometry_msgs/msg/Vector3Stamped
#   /starship/pose           geometry_msgs/msg/PoseStamped
#
# Run inside the ROS 2 container or with a sourced ROS 2 environment:
#   docker compose exec ros2 bash -ic "bash /ros2_ws/run_stub.sh"
#   -- or --
#   source /opt/ros/humble/setup.bash && bash examples/starship/stub/run_stub.sh
#
# The stub loops and republishes every ~2 s.  Ctrl-C to stop.

set -euo pipefail

echo "=== Starship telemetry stub starting ==="
echo "    Topics: /starship/{altitude,fuel_fraction,engine_state,velocity,pose}"
echo "    Press Ctrl-C to stop."
echo

ALTITUDE=450.0      # metres AGL (initial hover)
FUEL=0.62           # 62% propellant remaining
TICK=0

while true; do
    # Simulate gentle fuel burn and altitude drift
    FUEL=$(python3 -c "f=${FUEL}-0.002; print(f'{max(f,0.0):.3f}')")
    ALTITUDE=$(python3 -c "a=${ALTITUDE}+$(python3 -c 'import random; print(f\"{random.uniform(-2,2):.1f}\")'  ); print(f'{a:.1f}')")
    TICK=$((TICK + 1))

    # Engine state JSON (changes slowly)
    if (( TICK % 10 == 0 )); then
        ENGINE_STATUS="nominal"
    else
        ENGINE_STATUS="nominal"
    fi
    ENGINE_JSON="{\\\"status\\\": \\\"${ENGINE_STATUS}\\\", \\\"raptor_count\\\": 3, \\\"gimbal_ok\\\": true, \\\"rcs_ok\\\": true}"

    # altitude
    ros2 topic pub --once /starship/altitude std_msgs/msg/Float64 \
        "{data: ${ALTITUDE}}" > /dev/null 2>&1 &

    # fuel_fraction
    ros2 topic pub --once /starship/fuel_fraction std_msgs/msg/Float32 \
        "{data: ${FUEL}}" > /dev/null 2>&1 &

    # engine_state (JSON string)
    ros2 topic pub --once /starship/engine_state std_msgs/msg/String \
        "{data: '${ENGINE_JSON}'}" > /dev/null 2>&1 &

    # velocity (roughly hovering — small residuals)
    ros2 topic pub --once /starship/velocity geometry_msgs/msg/Vector3Stamped \
        "{header: {frame_id: 'world'}, vector: {x: 0.05, y: -0.03, z: 0.1}}" > /dev/null 2>&1 &

    # pose (at altitude above launch pad)
    ros2 topic pub --once /starship/pose geometry_msgs/msg/PoseStamped \
        "{header: {frame_id: 'world'}, pose: {position: {x: 0.0, y: 0.0, z: ${ALTITUDE}}, orientation: {w: 1.0}}}" > /dev/null 2>&1 &

    # status line
    printf "\r  [tick %3d]  altitude=%.1f m  fuel=%.3f" "${TICK}" "${ALTITUDE}" "${FUEL}"

    # Low fuel warning
    FUEL_WARN=$(python3 -c "print('LOW FUEL WARNING' if ${FUEL} < 0.10 else '')")
    if [[ -n "$FUEL_WARN" ]]; then
        printf "  \033[33m%s\033[0m" "$FUEL_WARN"
    fi
    if python3 -c "import sys; sys.exit(0 if ${FUEL} < 0.05 else 1)"; then
        printf "\n  \033[31mCRITICAL: fuel_fraction < 0.05 — FSW should engage safe mode!\033[0m\n"
    fi

    # Wait for background pubs to finish, then sleep
    wait
    sleep 2
done
