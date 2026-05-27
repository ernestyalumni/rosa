# ROSA Starship Demo — Laptop Setup Handoff
## For RTX 3070 Laptop (single GPU = display GPU = works with Isaac Sim)

**Context**: This was built on a desktop with GTX 980 Ti (display) + RTX 3060 (compute-only).
Isaac Sim couldn't render because the RTX 3060 wasn't connected to a display.
On the laptop the RTX 3070 IS the display GPU → Isaac Sim windowed mode should work.

**Interview**: Thu 2026-05-28 ~3pm PDT with Peter Toth, Matter Intelligence.

---

## Repos you need

| Repo | Branch | What it is |
|------|--------|-----------|
| `git@github.com:ernestyalumni/rosa.git` | `feat/rosa-isaac-crate` | ROSA Rust agent + Starship demo |
| `git@github.com:InServiceOfX/Monoclaw.git` | `feat/isaac-sim-ros2-bridge-fix` | Docker stacks: ROS2 + Isaac Sim |

---

## Step 0 — Prerequisites on the laptop

```bash
# Rust (if not installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env

# Docker
sudo apt install docker.io docker-compose-plugin
sudo usermod -aG docker $USER   # then log out/in

# NVIDIA Container Toolkit (for GPU passthrough to Docker)
curl -fsSL https://nvidia.github.io/libnvidia-container/gpgkey | sudo gpg --dearmor -o /usr/share/keyrings/nvidia-container-toolkit-keyring.gpg
curl -s -L https://nvidia.github.io/libnvidia-container/stable/deb/nvidia-container-toolkit.list | \
  sed 's#deb https://#deb [signed-by=/usr/share/keyrings/nvidia-container-toolkit-keyring.gpg] https://#g' | \
  sudo tee /etc/apt/sources.list.d/nvidia-container-toolkit.list
sudo apt update && sudo apt install -y nvidia-container-toolkit
sudo nvidia-ctk runtime configure --runtime=docker
sudo systemctl restart docker

# Verify
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi
```

---

## Step 1 — Clone repos

```bash
mkdir -p ~/workspace/repos
cd ~/workspace/repos

git clone git@github.com:ernestyalumni/rosa.git
cd rosa && git checkout feat/rosa-isaac-crate && cd ..

git clone git@github.com:InServiceOfX/Monoclaw.git
cd Monoclaw && git checkout feat/isaac-sim-ros2-bridge-fix && cd ..
```

---

## Step 2 — Set up API keys for ROSA

```bash
cd ~/workspace/repos/rosa
cp .env.example .env   # if .env.example exists, or create manually:
cat > .env << 'EOF'
# Priority: ANTHROPIC_API_KEY > XAI_API_KEY > OPENAI_API_KEY
XAI_API_KEY=<your-xai-api-key-here>
ROSA_MODEL=grok-4.3
EOF
```

---

## Step 3 — Build ROSA

```bash
cd ~/workspace/repos/rosa
cargo build --example starship -p rosa-cli
# Should print: Finished `dev` profile in ~2-3 minutes
```

---

## Step 4 — Start the ROS 2 container

```bash
cd ~/workspace/repos/Monoclaw/Deployments/Stacks/ROS
docker compose up -d
# Verify:
docker ps | grep rosa-ros2
```

If `rosa-ros2` container doesn't exist yet, the `docker compose up -d` will build it first (~5 min).

---

## Step 5 — Configure Isaac Sim for laptop

```bash
cd ~/workspace/repos/Monoclaw/Deployments/Stacks/IsaacSim
```

Edit `.env` to match the laptop (RTX 3070 is GPU 0 on a single-GPU laptop):

```bash
cat > .env << 'EOF'
ISAAC_VERSION=4.5.0
CONTAINER_NAME=isaac-sim
ROS_DOMAIN_ID=0
GPU_ID=0          # RTX 3070 is GPU 0 on laptop (single GPU)
DISPLAY=:1        # or :0 — check with: echo $DISPLAY
# windowed = full Isaac Sim GUI on your display (works because RTX 3070 IS the display GPU)
# headless = no window but HTTP API at :8282 (try this first)
ISAAC_MODE=windowed
OMNI_USER=admin
OMNI_PASS=admin
EOF
```

**Check your display number first:**
```bash
echo $DISPLAY   # usually :0 or :1 on laptop
```

### Pull the Isaac Sim image (15-25 GB — do this on WiFi or overnight)

```bash
# NGC login required (get API key from https://ngc.nvidia.com)
docker login nvcr.io
# Username: $oauthtoken
# Password: <your NGC API key>

docker pull nvcr.io/nvidia/isaac-sim:4.5.0
```

**If you already have the image on the desktop**, you can save/transfer:
```bash
# On desktop:
docker save nvcr.io/nvidia/isaac-sim:4.5.0 | gzip > /tmp/isaac-sim-4.5.0.tar.gz
# Transfer to laptop (rsync, USB, etc.), then on laptop:
docker load < isaac-sim-4.5.0.tar.gz
```

### Start Isaac Sim in windowed mode

```bash
cd ~/workspace/repos/Monoclaw/Deployments/Stacks/IsaacSim

# Allow Docker to access your X display
xhost +local:docker

# Build and start
docker compose build isaac   # first time only
docker compose up -d isaac

# Watch logs — Isaac Sim GUI should open on your desktop in ~30-60 seconds
docker logs -f isaac-sim 2>&1 | grep -E "rosa|ready|Error|clock|ROS"
```

**Expected**: An Isaac Sim window opens on your desktop. If it does, then:

```bash
# Load the Starship scene via HTTP API
curl -s http://localhost:8282/health   # should return {"status":"ok",...}
curl -X POST http://localhost:8282/starship/create-stage
curl -X POST http://localhost:8282/scene/load \
     -H "Content-Type: application/json" \
     -d '{"path":"/isaac-sim/exts/starship/starship.usd"}'
curl -X POST http://localhost:8282/timeline/play
```

---

## Step 6 — Run the demo

```bash
cd ~/workspace/repos/rosa
./run_demo.sh
```

This script:
1. Starts the Starship physics stub in `rosa-ros2` container
2. Pre-applies hover throttle 0.210 (full-fuel Mars hover)
3. Verifies 12 `/starship/*` ROS 2 topics are live
4. Launches the ROSA interactive REPL

---

## Demo prompts (see DEMO.md for full list)

```
> What is the current altitude, fuel fraction, and engine state?
> The vehicle is slowly climbing. Compute the precise hover throttle for the current fuel mass and apply it.
> Execute a controlled descent to 800 m, then hold station.
> ABORT! Emergency abort! Safe the vehicle immediately!
> Fuel is at 3% and we are at 400 m. What do you do?
> Reset the vehicle to Mars proximity ops position.
```

---

## Architecture summary (for the Claude session on the laptop)

### What's built

```
rosa/                                    (git: feat/rosa-isaac-crate)
├── crates/
│   ├── rosa-core/       Rust: Agent, Tool trait, ToolRegistry, LlmProvider
│   ├── rosa-llm/        Rust: AnthropicProvider + OpenAiProvider (xAI/Grok)
│   ├── rosa-tools/      Rust: generic tools
│   ├── rosa-ros2/       Rust: ros2_exec(), ros2_registry_default()
│   ├── rosa-isaac/      Rust: IsaacClient, HTTP tools (timeline/diagnostics/USD)
│   └── rosa-cli/
│       └── examples/
│           ├── starship.rs           ← MAIN DEMO FILE (600+ lines)
│           │   Tools: StarshipGetTelemetry, StarshipSetThrottle, StarshipSetGimbal,
│           │          StarshipFireRcs, StarshipSafeMode, StarshipReset,
│           │          StarshipSetGravityBody (HTTP to Isaac Sim :8282)
│           └── starship/stub/
│               ├── starship_sim_node.py   ← ROS2 physics stub (Mars, 1000m, 0.21 hover)
│               └── run_stub.sh
├── run_demo.sh          ← ONE-SHOT DEMO LAUNCHER
├── DEMO.md              ← Interview demo guide + talking points
└── ORCHESTRATION.md     ← Full architecture document

Monoclaw/Deployments/Stacks/            (git: feat/isaac-sim-ros2-bridge-fix)
├── ROS/
│   ├── docker-compose.yml    container: rosa-ros2, ROS 2 Humble, CycloneDDS
│   └── Dockerfile
└── IsaacSim/
    ├── docker-compose.yml    container: isaac-sim, NGC image, GPU_ID, network_mode:host
    ├── .env                  GPU_ID, ISAAC_MODE, DISPLAY
    ├── Dockerfile            base: nvcr.io/nvidia/isaac-sim:4.5.0
    ├── scripts/
    │   ├── start_isaac.sh    headless|windowed|streaming modes
    │   └── enable_ros2_bridge.py  HTTP API :8282, OmniGraph clock, physics endpoints
    └── starship/
        ├── create_stage.py   Generates starship.usd — Mars scene, physics, lighting
        ├── starship_v2.stl   4560-triangle mesh (ogive nose, 4 flaps, 6 Raptor bells)
        └── starship.usd      (generated by create_stage.py inside Isaac Sim container)
```

### Physics model (stub + Isaac Sim)
- Mars gravity: 3.72 m/s²
- Spawn: 1,000 m AGL, hover throttle 0.210 pre-applied
- Engines: 6× Raptor Vacuum, max 14.7 MN, 0–85% throttleable
- Fuel: 700,000 kg, burn rate 2,000 kg/s at 100%
- TVC gimbal: ±15° (±0.26 rad) pitch/yaw
- **Hover throttle formula**: T = (130000 + fuel_kg) × 3.72 / 14,700,000
  - Full fuel (830t): T ≈ 0.210
  - Empty (130t): T ≈ 0.033

### Key facts for the interview
1. ROSA is a **Rust-native LLM agent** replacing the Python/LangChain original
2. **Zero LangChain** — hand-rolled reqwest + SSE, type-safe Tool trait
3. **ROS 2 only** (ROS 1 dropped), runs entirely in Docker
4. The demo is against a Mars proximity ops physics stub
5. FSW rules enforced by the LLM: throttle limit 0.85, fuel<5%→safe_mode, ABORT→safe_mode
6. Isaac Sim provides USD physics + RTX rendering + PhysX rigid body

---

## Desktop vs Laptop differences

| Setting | Desktop | Laptop |
|---------|---------|--------|
| GPU_ID | 1 (RTX 3060, not display GPU) | 0 (RTX 3070, IS display GPU) |
| ISAAC_MODE | headless (hangs!) | windowed (should work) |
| Isaac Sim HTTP API | NOT AVAILABLE (hangs) | Available at :8282 |
| ROSA demo | stub only | stub + Isaac Sim tools |
| `run_demo.sh` | works fully | works fully |

### Desktop headless hang (DO NOT try to fix — just use windowed on laptop)
`SimulationApp({"headless": True})` hangs indefinitely after "app ready" on the desktop.
Root cause: RTX 3060 is not connected to a display → Vulkan EGL offscreen context
initialization hangs. In windowed mode it crashes with `vkCreateSwapchainKHR failed`
for the same reason. On laptop (RTX 3070 = display GPU), windowed mode should work.

---

## If Isaac Sim still doesn't open on the laptop

**Try headless mode** (sometimes works when the GPU is the display GPU):
```bash
# In IsaacSim/.env:
ISAAC_MODE=headless
```
Then check for HTTP API: `curl http://localhost:8282/health`

**If headless hangs too**, check:
```bash
docker logs -f isaac-sim 2>&1 | tail -20
# Look for: "[rosa] Isaac control API on http://0.0.0.0:8282"
# If you see "app ready" but nothing after, headless is also hanging
```

**Fall back to stub-only demo** (still excellent for the interview):
```bash
ISAAC_MODE=headless   # left as-is, not started
./run_demo.sh         # works without Isaac Sim
```

---

## Isaac Sim — loading the Starship stage (once GUI is open)

### Option A: Via HTTP API (if headless or headless+HTTP works)
```bash
# Generate the USD file inside the container
docker exec isaac-sim /isaac-sim/python.sh /isaac-sim/exts/starship/create_stage.py

# Load it
curl -X POST http://localhost:8282/scene/load \
     -H "Content-Type: application/json" \
     -d '{"path":"/isaac-sim/exts/starship/starship.usd"}'
curl -X POST http://localhost:8282/timeline/play
```

### Option B: Via Isaac Sim GUI (windowed mode)
1. Isaac Sim GUI opens on desktop
2. Menu: File → Open
3. Navigate to: `/isaac-sim/exts/starship/starship.usd`
4. Press ▶ Play in the timeline toolbar

### Option C: Via ROSA (if Isaac Sim tools active)
When running `./run_demo.sh` with Isaac Sim HTTP API up:
```
> Load the Starship stage and start the simulation
```
ROSA will call `load_usd` and `timeline_start` tools automatically.

---

## Quick sanity checks

```bash
# 1. GPU available to Docker
docker run --rm --gpus all nvidia/cuda:12.0-base nvidia-smi

# 2. ROS2 container running
docker ps | grep rosa-ros2

# 3. Stub topics
docker exec rosa-ros2 bash -c \
  "source /opt/ros/humble/setup.bash && ros2 topic list 2>/dev/null | grep starship"

# 4. Isaac Sim HTTP API (if running)
curl -s http://localhost:8282/health

# 5. ROSA compiles
cd ~/workspace/repos/rosa && cargo build --example starship -p rosa-cli
```

---

## Files to have open during interview

```bash
# Terminal 1: run_demo.sh → ROSA REPL
cd ~/workspace/repos/rosa && ./run_demo.sh

# Terminal 2: Isaac Sim logs (if running)
docker logs -f isaac-sim 2>&1 | grep -v Warning

# Editor tabs to show:
# - rosa/crates/rosa-cli/examples/starship.rs       (FSW tools, system prompt)
# - rosa/crates/rosa-cli/examples/starship/stub/starship_sim_node.py  (physics)
# - Monoclaw/Deployments/Stacks/IsaacSim/starship/create_stage.py     (USD scene)
# - rosa/ORCHESTRATION.md                           (architecture overview)
```
