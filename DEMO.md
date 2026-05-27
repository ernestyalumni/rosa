# ROSA Starship Demo — Matter Intelligence Interview
## Thursday 2026-05-28 ~3 PM PDT

---

## Quick Setup (5 min before call)

```bash
cd /home/propdev/.openclaw/workspace/workspace2/repos/rosa
./run_demo.sh
```

This script:
1. Ensures `rosa-ros2` container is running
2. Starts the Starship physics stub (Mars, 1000 m AGL, 0.21 hover throttle pre-applied)
3. Verifies all 12 `/starship/*` ROS 2 topics are live
4. Launches the ROSA interactive REPL

---

## Demo Prompts (use in order)

### 1. Read telemetry
```
> What is the current altitude, fuel fraction, and engine state?
```
*ROSA calls `starship_get_telemetry` → reads `/starship/altitude`, `/starship/engine_state`*  
*Expected: ~1000 m AGL, fuel 1.0, throttle 0.21*

### 2. Precise hover trim (GNC calculation demo)
```
> The vehicle is slowly climbing. Compute the precise hover throttle for the current fuel mass and apply it.
```
*ROSA computes T_hover = (130000 + fuel_kg) × 3.72 / 14700000 from actual telemetry*  
*Expected: ROSA calculates ~0.210 → 0.203 as fuel burns*

### 3. Controlled descent
```
> Execute a controlled descent from current altitude to 800 m, then hold station at 800 m.
```
*ROSA reduces throttle below hover, monitors altitude, makes corrections*  
*Expected: multiple tool calls (get_telemetry → set_throttle → get_telemetry ...)*

### 4. TVC gimbal burn (optional)
```
> Apply a 5-degree pitch gimbal to maneuver laterally for 3 seconds, then reset to neutral.
```
*ROSA converts degrees→radians: 5° = 0.087 rad, publishes to `/starship/main_gimbal`*

### 5. Emergency ABORT (crowd-pleaser)
```
> ABORT! Emergency abort! Safe the vehicle immediately!
```
*ROSA: get_telemetry → set_throttle(0) → safe_mode(true) → get_telemetry*  
*Expected: throttle=0.0, safe_mode=true, report, standing by*

### 6. Low-fuel emergency (FSW rule demo)
```
> Fuel is at 3% and we are at 400 m altitude. What do you recommend and execute?
```
*ROSA: FSW rule #3 fires: fuel<5% → safe_mode IMMEDIATELY, no questions*

### 7. Reset for new scenario
```
> Reset the vehicle to Mars proximity ops and prepare for another descent burn.
```
*ROSA calls `/starship/reset` service → altitude 1000 m, fuel 100%, hover throttle restored*

---

## Architecture Talking Points

### What is ROSA?
A **Rust-native LLM agent** that replaces the Python/LangChain original with:
- Type-safe tool registry (`Tool` trait → `ToolRegistry`)
- Streaming SSE rendering (show the `↗`/`↙` tool call arrows)
- Zero vendor SDK — hand-rolled reqwest + OpenAI-compatible API

### How does it work?
```
Operator prompt
      │
      ▼
  Agent::run_loop()
      │
      ├── LLM (grok-4.3/xAI or claude-sonnet/Anthropic)
      │       │ system prompt with FSW rules + GNC guidance
      │       │ tool schemas injected
      │       ▼
      ├── tool calls (JSON)
      │       │
      │       ▼
      └── ToolRegistry
              ├── starship_get_telemetry  → ros2 topic echo (docker exec)
              ├── starship_set_throttle   → ros2 topic pub /starship/main_throttle
              ├── starship_set_gimbal     → ros2 topic pub /starship/main_gimbal
              ├── starship_fire_rcs       → ros2 topic pub /starship/rcs/*
              ├── starship_safe_mode      → ros2 topic pub /starship/safe_mode
              ├── starship_reset          → ros2 service call /starship/reset
              └── starship_set_gravity    → HTTP POST :8282 (Isaac Sim)
```

### Physics model
- **Gravity**: Mars 3.72 m/s² (1/2.6 of Earth)
- **Spawn**: 1,000 m AGL, pre-engaged 0.21 hover throttle
- **Engines**: 6× Raptor Vacuum, max 14.7 MN, throttleable 0–85%
- **TVC**: ±15° gimbal, 5 m moment arm → torque
- **Fuel**: 700 t propellant, 2000 kg/s burn rate at 100%
- **RCS**: 3 groups for attitude control

### FSW rules enforced by the LLM
1. Always read telemetry before any command
2. NEVER set throttle > 0.85 (hard FSW limit)
3. Fuel < 5% → IMMEDIATE safe mode, no maneuvers
4. ABORT → throttle=0, safe_mode=true, standing by
5. Confirm state after EVERY command

### Isaac Sim integration (architecture, not live today)
- USD stage: Starship 3D mesh (50 m, stainless steel PBR), Martian regolith (10 km×10 km)
- Mars sun (2200 lux, low angle) + deep space dome light
- PhysX GPU rigid-body dynamics (6-DOF)
- HTTP API at :8282 for timeline control + gravity switching
- See: `Monoclaw/Deployments/Stacks/IsaacSim/starship/create_stage.py`

---

## Key Numbers

| Quantity | Value |
|----------|-------|
| Mars gravity | 3.72 m/s² |
| Spawn altitude | 1,000 m AGL |
| Hover throttle (full fuel 830 t) | 0.210 |
| Hover throttle (dry 130 t) | 0.033 |
| Max thrust | 14.7 MN |
| Hard throttle FSW limit | 0.85 |
| Fuel capacity | 700,000 kg |
| Burn rate at 100% | 2,000 kg/s |
| TVC gimbal range | ±15° (±0.26 rad) |
| FSW fuel emergency threshold | < 5% |

---

## Backup: Non-Interactive Test
If REPL has issues, test with piped input:
```bash
echo "What is the current altitude and fuel?" | \
  ROS_CONTAINER=rosa-ros2 cargo run --example starship -p rosa-cli
```

## Files to Show During Demo
- `ORCHESTRATION.md` — architecture overview
- `crates/rosa-cli/examples/starship.rs` — FSW tools (600+ lines, Rust)
- `crates/rosa-cli/examples/starship/stub/starship_sim_node.py` — Mars physics stub
- `Monoclaw/Deployments/Stacks/IsaacSim/starship/create_stage.py` — Isaac Sim USD stage
