<!-- Header block for project --> <hr>
<div align="center">
<!--   <img width="292" alt="ROSA_logo_dark_bg@2x" src="https://github.com/user-attachments/assets/7b4a8e64-9a08-4180-806a-5076d3672c05"> -->
<!--   <img width="213" alt="ROSA_sticker_color@2x" src="https://github.com/user-attachments/assets/5fa3a03e-5ef8-4942-84ac-95acf2f1777d"> -->
<!--   <img width="426" alt="ROSA_sticker_color@2x" src="https://github.com/user-attachments/assets/98b0a0ed-6b14-420c-83af-9067ab2d2d22"> -->
<!-- <img src="https://github.com/user-attachments/assets/d7175d5e-63d2-448c-b9d3-59ca0016ef7a"> -->
<img width="2057" alt="image" src="https://github.com/user-attachments/assets/ddbd3281-79f0-4d29-b0cd-30a5188ad061">

  
</div>
<div align="center">
  The ROS Agent (ROSA) is designed to interact with ROS-based<br>robotics systems using natural language queries. 🗣️🤖
</div>
<br>
<div align="center">

[![arXiv](https://img.shields.io/badge/arXiv-2410.06472-b31b1b.svg)](https://arxiv.org/abs/2410.06472)
![ROS 1](https://img.shields.io/badge/ROS_1-Noetic-blue)
![ROS 2](https://img.shields.io/badge/ROS_2-Humble|Iron|Jazzy-blue)
![License](https://img.shields.io/pypi/l/jpl-rosa)
[![SLIM](https://img.shields.io/badge/Best%20Practices%20from-SLIM-blue)](https://nasa-ammos.github.io/slim/)

![Main Branch](https://img.shields.io/github/actions/workflow/status/nasa-jpl/rosa/ci.yml?branch=main&label=main)
![Dev Branch](https://img.shields.io/github/actions/workflow/status/nasa-jpl/rosa/ci.yml?branch=dev&label=dev)
![Publish Status](https://img.shields.io/github/actions/workflow/status/nasa-jpl/rosa/publish.yml?label=publish)
![Version](https://img.shields.io/pypi/v/jpl-rosa)
![Downloads](https://img.shields.io/pypi/dw/jpl-rosa)

</div>
<!-- Header block for project -->

> [!IMPORTANT]
> 📚 **New to ROSA?** Check out our [Wiki](https://github.com/nasa-jpl/rosa/wiki) for documentation, guides and FAQs!

> [!WARNING]
> 🚧 **This fork (`ernestyalumni/rosa`) is under active Rust rewrite on branch `feat/rust-rewrite`.**
> LangChain has been removed. The Python `jpl-rosa` package is no longer published from this fork.
> See [`ORCHESTRATION.md`](ORCHESTRATION.md) for the rewrite plan and [`ARCHITECTURE.md`](ARCHITECTURE.md) for the Rust crate layout.

ROSA is an AI-powered assistant for ROS 2 systems that lets you interact with robots using natural language.
This fork replaces the upstream Python + LangChain implementation with a **Rust-first agent** (`rosa-cli`) that talks to ROS 2 via DDS — zero LangChain, zero host ROS install required (ROS 2 runs in Docker).

#### ROSA Demo: NeBula-Spot in JPL's Mars Yard (click for YouTube)
[![Spot YouTube Thumbnail](https://github.com/user-attachments/assets/19a99b5c-6103-4be4-8875-1810cf4558c5)](https://www.youtube.com/watch?v=mZTrSg7tEsA)


## 🚀 Quick Start (Rust rewrite)

### Requirements
- Rust stable (`rustup`)
- Docker + Docker Compose
- `ANTHROPIC_API_KEY` or `OPENAI_API_KEY`

### Run

```bash
# 1. Start the ROS 2 container
cd Monoclaw/Deployments/ROS && docker compose up -d

# 2. Build and run rosa
cd /path/to/rosa
cargo run --release -- ask "list ROS 2 topics"

# 3. Interactive REPL
cargo run --release
```

See [`ORCHESTRATION.md`](ORCHESTRATION.md) for the full build plan and sub-agent task briefs.


## Adapting ROSA for Your Robot 🔧

ROSA is designed to be easily adaptable to different robots and environments. You can create custom agents by either inheriting from the `ROSA` class or creating a new instance with custom parameters.

For detailed information on creating custom agents, adding tools, and customizing prompts, please refer to our [Custom Agents Wiki page](https://github.com/nasa-jpl/rosa/wiki/Custom-Agents).


## TurtleSim Demo 🐢

We have included a demo that uses ROSA to control the TurtleSim robot in simulation. To run the demo, you will need to have Docker installed on your machine. 🐳

The following video shows ROSA reasoning about how to draw a 5-point star, then 
executing the necessary commands to do so.

https://github.com/user-attachments/assets/77b97014-6d2e-4123-8d0b-ea0916d93a4e

For detailed instructions on setting up and running the TurtleSim demo, please refer to our [TurtleSim Demo Guide](https://github.com/nasa-jpl/rosa/wiki/Guide:-TurtleSim-Demo) in the Wiki.


## IsaacSim Extension (Coming Soon)

ROSA is coming to Nvidia IsaacSim! While you can already use ROSA with robots running in IsaacSim (using the ROS/ROS2 bridge), we are adding direct integration
in the form of an IsaacSim extension. This will allow you not only to control your robots in IsaacSim, but control IsaacSim itself. Check out the video below to learn mroe.

#### ROSA Demo: Nvidia IsaacSim Extension (click for YouTube)
[![Carter YouTube Thumbnail Play](https://github.com/user-attachments/assets/a6948d5e-2726-4dd8-8dee-19dfb5188f1d)](https://www.youtube.com/watch?v=mm5525G_EfQ)

## 📘 Learn More

- [📕 Read the paper](https://arxiv.org/abs/2410.06472)
- [🗺️ Roadmap](https://github.com/nasa-jpl/rosa/wiki/Feature-Roadmap)
- [🏷️ Releases](https://github.com/nasa-jpl/rosa/releases)
- [❓ FAQ](https://github.com/nasa-jpl/rosa/wiki/FAQ)


## Changelog

See our [CHANGELOG.md](CHANGELOG.md) for a history of our changes.  

## Contributing

Interested in contributing to our project? Please see our: [CONTRIBUTING.md](CONTRIBUTING.md)

For guidance on how to interact with our team, please see our code of conduct located
at: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)

For guidance on our governance approach, including decision-making process and our various roles, please see our
governance model at: [GOVERNANCE.md](GOVERNANCE.md)

## License

See our: [LICENSE](LICENSE)

## Support

Key points of contact are:

- [@RobRoyce](https://github.com/RobRoyce) ([email](mailto:01-laptop-voiced@icloud.com))

---

<div align="center">
  ROSA: Robot Operating System Agent 🤖<br>
  Copyright (c) 2024. Jet Propulsion Laboratory. All rights reserved.
</div>
