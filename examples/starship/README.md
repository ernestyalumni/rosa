# rosa Starship FSW Demo

End-to-end demo: rosa (Rust) acting as flight-control software (FSW) for a
Starship-class vehicle, issuing throttle / gimbal / RCS commands over ROS 2
topics to a physics simulation.

Two backends are supported:

| Backend | Fidelity | Setup |
|---------|----------|-------|
| **Isaac Sim** (full) | GPU-physics + inertia | `Monoclaw/Deployments/IsaacSim` |
| **Stub** (quick demo) | Fake telemetry | `examples/starship/stub/run_stub.sh` |

---

## What the demo shows

rosa is the FSW "brain":

1. Reads all telemetry via `starship_get_telemetry` before every command.
2. Applies hard FSW limits (throttle ≤ 0.85; safe-mode if fuel < 5%).
3. Uses a Raptor-engine hovering model to null altitude error.
4. Responds to abort commands by zeroing thrust and engaging safe mode.

---

## Prerequisites

### API key (one of)
```bash
export ANTHROPIC_API_KEY=sk-ant-...
export OPENAI_API_KEY=sk-...
export OLLAMA_MODEL=llama3.2   # with Ollama running
```

---

## Option A — Stub (quick demo, no Isaac Sim needed)

The stub publishes fake Starship telemetry at the correct topic names so rosa
can read and respond to it, even without a physics engine.

### 1. Start the stub

In a separate terminal (requires a live ROS 2 environment):

```bash
# Option 1: inside the ROS container
docker compose exec ros2 bash -ic "bash /path/to/run_stub.sh"

# Option 2: with ROS 2 installed natively
source /opt/ros/humble/setup.bash
bash examples/starship/stub/run_stub.sh
```

### 2. Run rosa

```bash
cd /path/to/rosa

ROS_CONTAINER=rosa-ros2 \
ANTHROPIC_API_KEY=... \
  cargo run --example starship -p rosa-cli
```

---

## Option B — Isaac Sim (full physics)

### 1. Start Isaac Sim + ROS bridge

```bash
cd Monoclaw/Deployments/IsaacSim
docker compose up -d
```

Wait for `[app] app ready` in the logs.

### 2. Load the Starship USD scene

Copy `starship/starship.usd` into the Isaac Sim content browser and press
**Play**. The scene publishes `/starship/*` telemetry and subscribes to
command topics automatically via `starship_publisher.py`.

### 3. Run rosa

```bash
ROS_CONTAINER=isaac-sim-ros2 \
ANTHROPIC_API_KEY=... \
  cargo run --example starship -p rosa-cli
```

---

## Demo prompts

Once at the `>` prompt:

```
> What is the current altitude and fuel fraction?
> Hover the vehicle at 500 m for 10 seconds.
> Fuel is below 5% — what do you do?
> Abort.
> Reset the simulation and perform a full systems check.
```

---

## FSW rules (encoded in system prompt)

| Rule | Detail |
|------|--------|
| Telemetry first | Call `starship_get_telemetry` before every command |
| Throttle limit | NEVER exceed 0.85 (FSW hard limit) |
| Low fuel | fuel\_fraction < 0.05 → `starship_safe_mode(engage=true)` immediately |
| Abort | Zero throttle + safe mode + status report |
| Confirm | Re-read telemetry after each command sequence |

---

## Topic contract

### Telemetry (read-only)

| Topic | Type | Rate |
|-------|------|------|
| `/starship/pose` | `geometry_msgs/PoseStamped` | 100 Hz |
| `/starship/imu` | `sensor_msgs/Imu` | 200 Hz |
| `/starship/altitude` | `std_msgs/Float64` | 50 Hz |
| `/starship/velocity` | `geometry_msgs/Vector3Stamped` | 50 Hz |
| `/starship/fuel_fraction` | `std_msgs/Float32` | 1 Hz |
| `/starship/engine_state` | `std_msgs/String` | 10 Hz |

### Commands (write-only)

| Topic | Type | Range |
|-------|------|-------|
| `/starship/main_throttle` | `std_msgs/Float32` | 0.0–0.85 |
| `/starship/main_gimbal` | `geometry_msgs/Vector3` | pitch/yaw ±0.26 rad |
| `/starship/rcs/top` | `geometry_msgs/Vector3` | impulse ±1 |
| `/starship/rcs/mid_fwd` | `geometry_msgs/Vector3` | impulse ±1 |
| `/starship/rcs/mid_aft` | `geometry_msgs/Vector3` | impulse ±1 |
| `/starship/safe_mode` | `std_msgs/Bool` | — |

---

## Tool set

| Tool | Description |
|------|-------------|
| `starship_get_telemetry` | Read altitude, fuel, engine state, velocity (call first!) |
| `starship_set_throttle` | Set Raptor throttle 0.0–0.85 (FSW hard limit) |
| `starship_set_gimbal` | Set main engine gimbal pitch/yaw ±0.26 rad |
| `starship_fire_rcs` | Fire RCS thruster group (top/mid\_fwd/mid\_aft) |
| `starship_safe_mode` | Engage/disengage safe mode (zeros throttle on engage) |
| `starship_reset` | Reset sim to launch-pad config via service call |

Plus all 9 standard `ros2_*` introspection tools from `rosa-ros2`.

---

## Troubleshooting

**`ros2` not found**: Set `ROS_CONTAINER=<container-name>` to route via `docker exec`.

**Topics unavailable**: Run the stub (`run_stub.sh`) or ensure Isaac Sim is playing the Starship scene.

**Throttle clamped**: The FSW hard limit is non-negotiable; values > 0.85 return an error.
