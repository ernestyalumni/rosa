# rosa-isaac

Isaac Sim integration tools for the ROSA Rust agent.

Provides 6 agent tools that talk to the HTTP control API embedded inside
`Monoclaw/Deployments/Stacks/IsaacSim/scripts/enable_ros2_bridge.py`,
which runs alongside Isaac Sim's headless simulation loop.

## Tools

| Tool             | HTTP endpoint       | Effect                            |
|------------------|---------------------|-----------------------------------|
| `timeline_start` | `POST /timeline/play`  | Start simulation; `/clock` begins ticking |
| `timeline_stop`  | `POST /timeline/stop`  | Stop and rewind to t=0            |
| `timeline_pause` | `POST /timeline/pause` | Freeze simulation (keep state)    |
| `get_diagnostics`| `GET  /diagnostics`    | `{fps, sim_time, running, physics_dt}` |
| `load_usd`       | `POST /scene/load`     | Load a USD scene file             |
| `list_usds`      | `GET  /scene/list`     | Discover available USD scenes     |

## Quick start

```rust
use rosa_isaac::{IsaacClient, tools::all_isaac_tools};
use rosa_tools::ToolRegistry;

// Connect to default URL http://localhost:8282
let client = IsaacClient::default();

// Or use a custom URL:
// let client = IsaacClient::new("http://isaac-host:8282");

// Register all isaac tools into an existing registry
let registry = all_isaac_tools(ToolRegistry::new(), client);
```

## Environment variables

| Variable            | Default                    | Description                         |
|---------------------|----------------------------|-------------------------------------|
| `ISAAC_CONTROL_URL` | `http://localhost:8282`    | Base URL of the Isaac control API   |

## Integration with starship.rs

`crates/rosa-cli/examples/starship.rs` auto-detects Isaac Sim:

```bash
# With Isaac Sim running (port 8282 reachable)
ROS_CONTAINER=rosa-ros2 ANTHROPIC_API_KEY=... cargo run --example starship -p rosa-cli

# With explicit URL
ISAAC_CONTROL_URL=http://localhost:8282 ... cargo run --example starship -p rosa-cli
```

The starship example prints whether Isaac tools were added at startup:
```
  isaac  → http://localhost:8282 (timeline + diagnostics + USD tools enabled)
```
or:
```
  isaac  → not detected at localhost:8282 (timeline tools disabled)
           set ISAAC_CONTROL_URL to enable or start Isaac Sim
```

## Isaac Sim setup

The HTTP control server is embedded in `enable_ros2_bridge.py`.
Start it via the docker-compose stack:

```bash
cd Monoclaw/Deployments/Stacks/IsaacSim
docker compose up -d
# Wait ~5-30 min for Isaac Sim to boot (first boot compiles shaders)
curl http://localhost:8282/health
# {"status": "ok"}
```

See `Monoclaw/Deployments/Stacks/IsaacSim/scripts/test_control_api.sh`
for a full smoke test.
