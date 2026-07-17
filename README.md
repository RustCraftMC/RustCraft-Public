# RustCraft

[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)](https://www.rust-lang.org/)  
[![Vulkan](https://img.shields.io/badge/Renderer-Vulkan-blue)](https://chatgpt.com/c/6a59e0a4-7d3c-83e9-a6c2-7fd31e387b6e#)  
[![Protocol](https://img.shields.io/badge/Minecraft-1.8.9%20\(protocol%2047\)-green)](https://chatgpt.com/c/6a59e0a4-7d3c-83e9-a6c2-7fd31e387b6e#)  
[![Build](https://img.shields.io/badge/Build-Cargo-informational)](https://chatgpt.com/c/6a59e0a4-7d3c-83e9-a6c2-7fd31e387b6e#)

**English** | [简体中文](https://chatgpt.com/c/README_ZH.md)

> A Minecraft 1.8.9 client reimplementation built from scratch in Rust with a Vulkan renderer.

RustCraft is an experimental reimplementation of the **Minecraft Java Edition 1.8.9 client**, written primarily in **Rust 2021** and powered by a custom **Vulkan** rendering engine.

The goal of RustCraft is to explore a modern approach to building a Minecraft-compatible client using Rust, explicit graphics APIs, and a modular architecture designed with performance and extensibility in mind.

> [!NOTE]  
> RustCraft is under active development. Many features are experimental, incomplete, or subject to change.

## Features

- Written primarily in Rust 2021

- Custom Vulkan-based rendering engine

- Minecraft 1.8.9 protocol 47 support

- World and chunk rendering

- Block and entity rendering

- Multiplayer server connectivity

- Microsoft account authentication

- Resource pack support

- Custom UI system

- Audio system

- Client-side Lua scripting and modding

- Modular game and rendering architecture

- Windows and Linux support

- Automated builds with GitHub Actions

- Linux AppImage packaging

- Shader pack support — **Work in Progress**

## Technology

RustCraft is primarily built with:

- **Rust 2021** — Core client and engine implementation

- **Vulkan** — Low-level graphics rendering

- **Lua** — Client-side scripting and modding

- **Cargo** — Dependency management and build system

- **Minecraft Protocol 47** — Minecraft Java Edition 1.8.9 networking

RustCraft does not use the original Minecraft Java client as its runtime. Core client, game, networking, and rendering systems are independently implemented in Rust.

Minecraft-owned resources are not distributed with RustCraft. Users must provide the required resources from their own legally obtained Minecraft installation.

## Project Structure

The public repository contains the RustCraft source code and project-owned resources required for development.

```
RustCraft-Public/
├── assets/
│   └── RustCraft-owned resources only
├── src/
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
└── README_ZH.md
```

Minecraft-owned assets, cached resources, downloaded game files, textures, language files, and other third-party copyrighted content are intentionally excluded from the public repository.

## Building

### Requirements

To build RustCraft, you will need:

- A recent stable Rust toolchain

- Cargo

- A Vulkan-compatible GPU

- Up-to-date graphics drivers

- Vulkan development libraries or the Vulkan SDK

- A legally obtained installation of Minecraft Java Edition 1.8.9

Clone the repository:

```
git clone https://github.com/YOUR_USERNAME/RustCraft-Public.git
cd RustCraft-Public
```

Build the release version:

```
cargo build --release --locked
```

The compiled executable will be available at:

Windows:

```
target/release/rustcraft.exe
```

Linux:

```
target/release/rustcraft
```

## Preparing Minecraft Assets

RustCraft requires Minecraft's original game assets and resources in order to start and render the game correctly.

For copyright and licensing reasons, these files are **not included** in the RustCraft repository or release packages.

You must obtain them from your own legally installed copy of Minecraft Java Edition.

There are two types of Minecraft resources that need to be prepared:

1. Assets downloaded and managed by the official Minecraft Launcher

2. Resources contained inside the Minecraft 1.8.9 client JAR

### 1. Copy the Official Launcher Assets

First, install and launch **Minecraft Java Edition 1.8.9** at least once using the official Minecraft Launcher.

This ensures that the required asset indexes and object files are downloaded.

Locate your Minecraft installation directory.

On Windows:

```
%APPDATA%\.minecraft
```

On Linux:

```
~/.minecraft
```

On macOS:

```
~/Library/Application Support/minecraft
```

Inside the Minecraft directory, locate:

```
assets/
```

The directory normally contains files and directories such as:

```
assets/
├── indexes/
├── objects/
└── ...
```

Copy the required contents from this `assets` directory into the `assets` directory used by RustCraft.

For a prebuilt Windows release, the resulting layout should look similar to:

```
RustCraft/
├── rustcraft.exe
└── assets/
    ├── indexes/
    ├── objects/
    └── ...
```

When running RustCraft from source, place the required assets in the repository's `assets` directory:

```
RustCraft-Public/
├── assets/
│   ├── indexes/
│   ├── objects/
│   └── ...
├── src/
├── Cargo.toml
└── Cargo.lock
```

> [!IMPORTANT]  
> Do not download Minecraft assets from unofficial third-party mirrors. Use the files downloaded by your own official Minecraft installation.

### 2. Extract Resources from the Minecraft 1.8.9 JAR

RustCraft may also require resources that are bundled directly inside the Minecraft 1.8.9 client JAR.

First, make sure Minecraft Java Edition 1.8.9 has been installed through the official Minecraft Launcher.

The Minecraft 1.8.9 JAR is normally located at:

Windows:

```
%APPDATA%\.minecraft\versions\1.8.9\1.8.9.jar
```

Linux:

```
~/.minecraft/versions/1.8.9/1.8.9.jar
```

macOS:

```
~/Library/Application Support/minecraft/versions/1.8.9/1.8.9.jar
```

A JAR file is based on the ZIP archive format. You can open or extract it using a compatible archive utility.

Extract the Minecraft resource directories required by RustCraft from your own `1.8.9.jar`.

Minecraft 1.8.9 resources inside the JAR are typically located under:

```
assets/minecraft/
```

Copy the required resource files into RustCraft's asset directory while preserving their directory structure.

For example, the resulting structure may look similar to:

```
RustCraft/
└── assets/
    ├── indexes/
    ├── objects/
    └── minecraft/
        ├── blockstates/
        ├── lang/
        ├── models/
        ├── shaders/
        └── textures/
```

The exact set of required resources may change as RustCraft development progresses.

> [!IMPORTANT]  
> You must extract these resources from your own legally obtained Minecraft 1.8.9 installation. RustCraft does not provide, host, or redistribute the Minecraft 1.8.9 JAR or Minecraft-owned game assets.

## Running

Once the required Minecraft assets have been prepared, RustCraft can be started normally.

When running from source:

```
cargo run --release
```

When using a prebuilt Windows package, launch:

```
rustcraft.exe
```

Make sure the required `assets` directory is available in the runtime location expected by RustCraft.

If RustCraft fails to start or displays missing textures, models, sounds, or language resources, verify that:

- Minecraft Java Edition 1.8.9 has been launched at least once using the official launcher.

- The required official launcher assets have been copied correctly.

- The required resources from `1.8.9.jar` have been extracted.

- The original directory structure has been preserved.

- RustCraft can access the `assets` directory from its current working directory.

## Resource Packs

RustCraft supports Minecraft resource packs.

Resource pack ZIP files can be placed in:

```
resourcepacks/
```

Resource packs can be used to customize supported game resources without modifying RustCraft's source code.

Minecraft resource packs and original Minecraft assets are not distributed as part of the RustCraft repository.

## Shader Packs

> [!WARNING]  
> Shader pack support is currently a **Work in Progress**.

RustCraft is working toward shader pack support through its custom Vulkan rendering pipeline.

The implementation is currently experimental and incomplete. Compatibility with existing Minecraft shader packs is not guaranteed, and the shader pack system may change significantly during development.

## Lua Modding

RustCraft includes a client-side Lua scripting system designed to provide a flexible way to extend and customize the client.

The scripting system is intended to expose controlled game APIs for areas such as:

- Camera

- World

- Entities

- Player

- Inventory

- Chat

The Lua API and modding capabilities are under active development and may change between versions.

## Platform Support

| Platform | Architecture | Package   |
| -------- | ------------ | --------- |
| Windows  | x86_64       | ZIP / EXE |
| Linux    | x86_64       | AppImage  |

Other platforms and architectures are currently not officially supported.

## Downloads

Prebuilt packages are automatically generated through GitHub Actions.

Development builds are available as GitHub Actions artifacts.

Versioned releases are automatically published when a version tag matching `v*` is pushed.

For example:

```
v0.1.0
v0.2.0
v1.0.0
```

Release packages do **not** include Minecraft-owned assets or the Minecraft client JAR.

Users must provide the required resources from their own legally obtained Minecraft installation.

## Development Status

RustCraft is an experimental project and is not yet considered production-ready.

Current development focuses on:

- Improving rendering correctness

- Optimizing Vulkan performance

- Improving Minecraft 1.8.9 compatibility

- Expanding multiplayer protocol support

- Improving resource pack compatibility

- Expanding the Lua scripting API

- Improving cross-platform support

- Developing shader pack support

Bug reports and contributions are welcome.

## Legal Notice

RustCraft is an independent, unofficial project.

RustCraft is **not affiliated with, endorsed by, sponsored by, or officially associated with Mojang Studios or Microsoft**.

Minecraft and related names and assets are trademarks and copyrighted materials of their respective owners.

This repository does not distribute Minecraft-owned game assets, the Minecraft client JAR, or other proprietary Minecraft content.

Users are responsible for obtaining and using Minecraft resources from their own legally obtained copy of Minecraft in accordance with the applicable terms and licenses.

## License

RustCraft is distributed under the **RustCraft Noncommercial Source-Available License**.

In summary:

- Personal and noncommercial use is permitted.

- Studying and modifying the source code is permitted.

- Private modifications for personal, noncommercial use are permitted.

- Commercial use is prohibited without separate written permission.

- Closed-source distribution of modified versions is prohibited.

- If a modified version is distributed, its corresponding source code must be made publicly available.

- Minecraft-owned and other third-party assets are not covered by the RustCraft license.

See the [LICENSE](https://chatgpt.com/c/LICENSE) file for the complete license terms.
