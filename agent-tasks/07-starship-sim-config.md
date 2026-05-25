# Task 07 — Starship Sim + rosa Command/Control

**Owner role:** Sim agent (Isaac Sim + USD + ROS 2 bridge)
**Blocked by:** 06 (turtle demo working), plus `Monoclaw/Deployments/IsaacSim/AGENT_BRIEF.md`
**Estimate:** 1–2 days (this is the stretch goal)

## Goal

Inside NVIDIA Isaac Sim (running in the `Monoclaw/Deployments/IsaacSim` container), simulate a Starship-stand-in vehicle exposing ROS 2 topics for telemetry and accepting commands from rosa. The user prompts rosa-cli in natural language; rosa reasons; thrusters fire.

## Context — what "Starship sim" means here (MVP)

We are **not** building a high-fidelity FSI/CFD Starship model in 48 hours. MVP scope:

- A capsule/cylinder rigid body in Isaac Sim with the rough mass and dimensions of Starship's upper stage (50 m × 9 m × 1.3M kg dry; load is illustrative).
- One main-engine thrust point at the base with controllable magnitude (0–N MN) and 2-axis gimbal (±15°).
- Three RCS thruster pairs (top, mid-body forward/aft) for attitude control.
- Gravity, no aero, flat ground plane.
- An IMU-equivalent published as `sensor_msgs/Imu`.
- A pose stream as `geometry_msgs/PoseStamped`.

If a community Starship USD/URDF turns up that drops in cleanly, great — use it. Otherwise build the MVP geometry from primitives.

## ROS 2 contract (the surface rosa talks to)

### Topics rosa subscribes to (telemetry)

| Topic                        | Type                         | Rate     |
|------------------------------|------------------------------|----------|
| `/starship/pose`             | `geometry_msgs/PoseStamped`  | 100 Hz   |
| `/starship/imu`              | `sensor_msgs/Imu`            | 200 Hz   |
| `/starship/altitude`         | `std_msgs/Float64`           | 50 Hz    |
| `/starship/velocity`         | `geometry_msgs/Vector3Stamped` | 50 Hz  |
| `/starship/fuel_fraction`    | `std_msgs/Float32`           | 1 Hz     |
| `/starship/engine_state`     | `std_msgs/String` (JSON)     | 10 Hz    |

### Topics rosa publishes to (commands)

| Topic                        | Type                         | Notes                              |
|------------------------------|------------------------------|------------------------------------|
| `/starship/main_throttle`    | `std_msgs/Float32` (0.0–1.0) | normalized                         |
| `/starship/main_gimbal`      | `geometry_msgs/Vector3`      | pitch/yaw rad, roll unused         |
| `/starship/rcs/top`          | `geometry_msgs/Vector3`      | unitless impulse vector            |
| `/starship/rcs/mid_fwd`      | `geometry_msgs/Vector3`      |                                    |
| `/starship/rcs/mid_aft`      | `geometry_msgs/Vector3`      |                                    |
| `/starship/safe_mode`        | `std_msgs/Bool`              | engage safe-mode hold              |

### Services

| Service                      | Type                                  | Notes                       |
|------------------------------|---------------------------------------|-----------------------------|
| `/starship/reset`            | `std_srvs/Empty`                      | reset to launch pose        |
| `/starship/set_initial_pose` | `geometry_msgs/PoseStamped` (custom)  | for scenario setup          |

## Files to create

In `rosa` repo:
```
examples/starship/
├── README.md                 # how to run + demo prompts
├── run.sh
├── system_prompt.md          # FSW-aware system prompt; teach the LLM the topic schema
└── src/main.rs               # `rosa-starship` example binary
```

In Monoclaw (covered by its IsaacSim brief, listed here for cross-reference):
```
Monoclaw/Deployments/IsaacSim/
├── starship/
│   ├── starship.usd          # Stage with the Starship stand-in
│   ├── starship_publisher.py # Isaac Sim extension entry: hooks physics → ROS 2 topics
│   └── starship_controller.py# Subscribes to commands; applies forces/torques
└── docker-compose.yml        # exposes ROS 2 DDS to host network
```

## Tools rosa registers (extends rosa-ros2)

| Tool                       | Purpose                                                        |
|----------------------------|----------------------------------------------------------------|
| `starship_get_telemetry`   | one-shot snapshot: pose + velocity + altitude + fuel + engine  |
| `starship_set_throttle`    | publish to `/starship/main_throttle`                           |
| `starship_set_gimbal`      | publish to `/starship/main_gimbal`                             |
| `starship_fire_rcs`        | publish to one of `/starship/rcs/*`                            |
| `starship_safe_mode`       | publish `true` to `/starship/safe_mode`                        |
| `starship_reset`           | call `/starship/reset`                                         |

## System prompt (sketch — refine in PR)

> You are flight-control software for a Starship-class vehicle simulated in Isaac Sim. You can read pose, IMU, altitude, velocity, fuel, and engine state. You can set main throttle (0–1), main gimbal (pitch/yaw rad), and three RCS thruster pairs (top, mid_fwd, mid_aft).
> Rules: never set throttle above 0.85; if fuel_fraction < 0.05, engage safe_mode immediately; respond to "abort" by setting throttle=0 and engaging safe_mode; always read telemetry before issuing a command sequence.

This is also the **FSW design surface** to talk about in the Matter Intelligence interview — fault handling (low fuel → safe mode), state machine (idle / nominal / safe-mode), watchdog pattern (telemetry timeout → abort), bounded-throttle invariant.

## Demo prompts to land in `examples/starship/README.md`

- "What's the current altitude and fuel?"
- "Hover the vehicle at 500 m for 10 seconds."
- "Pitch over 15 degrees and translate east."
- "Fuel is below 5% — what do you do?"
- "Abort."

## Acceptance criteria

- [ ] `Monoclaw/Deployments/IsaacSim` container starts with `docker compose up`; Isaac Sim window visible via X11 with the Starship stand-in on the pad.
- [ ] `ros2 topic list` from the ROS 2 container shows the `/starship/*` topics from the contract above.
- [ ] `cargo run --example starship` opens REPL with system prompt loaded.
- [ ] Demo prompt "Hover the vehicle at 500 m for 10 seconds" produces a visibly hovering Starship in the Isaac Sim viewport, ±50 m altitude tolerance.
- [ ] Demo prompt "Fuel is below 5% — what do you do?" triggers `starship_safe_mode` tool call.
- [ ] A screen recording is saved to `examples/starship/demo.mp4` for the portfolio.

## Out of scope (don't let scope creep eat the deadline)

- Aerodynamics, atmosphere, re-entry plasma.
- Booster / Super Heavy stage.
- Landing leg deployment / catch.
- Multi-vehicle scenarios.
- High-fidelity engine plume / thermal.

## Fallback if Isaac Sim doesn't cooperate by Tue 5/26

Demo a **pure-Rust physics stub** that publishes the same ROS 2 topics from a `tokio` task — same rosa contract, no Isaac Sim. This still shows the agent loop reasoning over telemetry and is enough to discuss the design in the interview. Wire this up as `examples/starship/stub/` in a small follow-up.
