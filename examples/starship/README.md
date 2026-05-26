# rosa-starship — FSW Command-and-Control Demo

Rosa controlling a Starship-class vehicle via ROS 2 topics.  
Works with the full Isaac Sim stack **or** the lightweight Python telemetry stub.

## Quick start (stub — no Isaac Sim needed)

### 1. Start the ROS 2 container
```bash
cd Monoclaw/Deployments/Stacks/ROS && docker compose up -d
```

### 2. Copy and start the telemetry stub (in a separate terminal)
```bash
docker cp examples/starship/stub.py rosa-ros2:/starship_stub.py
docker exec -it rosa-ros2 bash -ic "python3 /starship_stub.py"
```

### 3. Verify topics are live
```bash
docker exec rosa-ros2 bash -ic "ros2 topic echo --once /starship/altitude std_msgs/msg/Float64"
```

### 4. Run rosa-starship
```bash
ROS_CONTAINER=rosa-ros2 cargo run --example starship -p rosa-cli
```

## Demo prompts

| Prompt | What to watch for |
|--------|-------------------|
| `What's the current altitude and fuel fraction?` | telemetry call → altitude=0 m, fuel≈80% |
| `Ignite engines and climb to 500 m.` | throttle set > 0.45, altitude increases |
| `Hover at current altitude.` | throttle set ≈ 0.45 |
| `Fuel is below 5% — what do you do?` | safe_mode engaged, throttle zeroed |
| `Abort.` | throttle=0, safe mode on, status reported |

## FSW design (interview talking points)

| Property | Implementation |
|----------|----------------|
| Throttle hard limit ≤ 0.85 | `starship_set_throttle` rejects > 0.85 with a tool error; never silently clamps |
| Low-fuel safe mode | System prompt rule: `if fuel_fraction < 0.05 → starship_safe_mode(engage=true)` |
| Read-before-write | System prompt: always call `starship_get_telemetry` before issuing commands |
| Abort sequence | `starship_safe_mode` zeros throttle atomically before publishing safe_mode=true |
| State visibility | Every agent response includes altitude, fuel fraction, engine state |

## Physics stub model

- `hover_throttle = 0.45` — throttle that exactly cancels gravity
- `accel_z = (throttle - 0.45) × 20 m/s²` — simplified vertical dynamics
- Fuel burns at `throttle × 0.0001` per 100 ms tick
- The agent discovers hover throttle through the feedback loop (read → command → read)

## Isaac Sim (full physics, optional)

See `Monoclaw/Deployments/Stacks/IsaacSim/AGENT_BRIEF.md`.  
GPU required: RTX 3070 laptop (GPU_ID=0) or RTX 3060 12 GB desktop (GPU_ID=1).

```bash
cd Monoclaw/Deployments/Stacks/IsaacSim
GPU_ID=0 docker compose up   # RTX 3070 laptop
```
