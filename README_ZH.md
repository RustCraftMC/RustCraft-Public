# RustCraft

[![Rust](https://img.shields.io/badge/Rust-2021-orange?logo=rust)](https://www.rust-lang.org/)
[![Vulkan](https://img.shields.io/badge/Renderer-Vulkan-blue)](#)
[![Protocol](https://img.shields.io/badge/Minecraft-1.8.9%20(protocol%2047)-green)](#)
[![Build](https://img.shields.io/badge/Build-Cargo-informational)](#)

[English](README.md) | **简体中文**

## Star History

<a href="https://www.star-history.com/?repos=RustCraftMC%2FRustCraft-Public&type=date&legend=top-left">
 <picture>
   <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/chart?repos=RustCraftMC/RustCraft-Public&type=date&theme=dark&legend=top-left&sealed_token=grGCoVN8LZatfwjgJlrVzd7-9Te6P94gd4VFAcvN-nSB2CeJDFhton4LyL0svRIo6uEL8_j1zTlMVynfQcrdcvGO66C1fW55YBdXpWu_vZ4dmJDyosTWP1Wt1lYf3gsGf9-SjCMoBoCJj7SgGRU_RN0ZKcC2YcO7jIbBZ6uaW3RAsMsy9-K8XIXHpCqG" />
   <source media="(prefers-color-scheme: light)" srcset="https://api.star-history.com/chart?repos=RustCraftMC/RustCraft-Public&type=date&legend=top-left&sealed_token=grGCoVN8LZatfwjgJlrVzd7-9Te6P94gd4VFAcvN-nSB2CeJDFhton4LyL0svRIo6uEL8_j1zTlMVynfQcrdcvGO66C1fW55YBdXpWu_vZ4dmJDyosTWP1Wt1lYf3gsGf9-SjCMoBoCJj7SgGRU_RN0ZKcC2YcO7jIbBZ6uaW3RAsMsy9-K8XIXHpCqG" />
   <img alt="Star History Chart" src="https://api.star-history.com/chart?repos=RustCraftMC/RustCraft-Public&type=date&legend=top-left&sealed_token=grGCoVN8LZatfwjgJlrVzd7-9Te6P94gd4VFAcvN-nSB2CeJDFhton4LyL0svRIo6uEL8_j1zTlMVynfQcrdcvGO66C1fW55YBdXpWu_vZ4dmJDyosTWP1Wt1lYf3gsGf9-SjCMoBoCJj7SgGRU_RN0ZKcC2YcO7jIbBZ6uaW3RAsMsy9-K8XIXHpCqG" />
 </picture>
</a>

> 一个使用 Rust 从零重新实现、基于 Vulkan 渲染器的 Minecraft 1.8.9 客户端。

RustCraft 是一个实验性的 **Minecraft Java Edition 1.8.9 客户端重新实现项目**，主要使用 **Rust 2021** 编写，并采用自研的 **Vulkan** 渲染引擎。

RustCraft 的目标是探索使用现代系统编程语言、显式图形 API 和模块化架构重新构建 Minecraft 兼容客户端的可能性，同时注重性能、可扩展性和现代化的渲染架构。

> [!NOTE]  
> RustCraft 目前仍处于积极开发阶段。许多功能仍为实验性功能、尚未完成，或可能在未来发生较大变化。

## 功能特性

- 主要使用 Rust 2021 编写

- 自研 Vulkan 渲染引擎

- 支持 Minecraft 1.8.9 Protocol 47

- 世界与区块渲染

- 方块与实体渲染

- 多人游戏服务器连接

- Microsoft 账户登录

- 资源包支持

- 自定义 UI 系统

- 音频系统

- 客户端 Lua 脚本与 Modding 系统

- 模块化游戏与渲染架构

- 支持 Windows 与 Linux

- 使用 GitHub Actions 自动构建

- Linux AppImage 打包

- Shader Pack 支持 — **开发中（WIP）**

## 技术栈

RustCraft 主要使用以下技术：

- **Rust 2021** — 核心客户端与引擎实现

- **Vulkan** — 底层图形渲染

- **Lua** — 客户端脚本与 Modding

- **Cargo** — 依赖管理与构建系统

- **Minecraft Protocol 47** — Minecraft Java Edition 1.8.9 网络协议

RustCraft 运行时不依赖原版 Minecraft Java 客户端。核心客户端、游戏逻辑、网络和渲染系统均使用 Rust 独立实现。

RustCraft 不分发 Minecraft 原版资源。用户必须从自己合法获取的 Minecraft 安装中提供运行所需的资源文件。

## 项目结构

公开仓库包含 RustCraft 的源代码以及项目自身拥有版权的开发资源。

```
RustCraft-Public/
├── assets/
│   └── 仅包含 RustCraft 自有资源
├── src/
├── Cargo.lock
├── Cargo.toml
├── LICENSE
├── README.md
└── README_ZH.md
```

Minecraft 原版 assets、缓存资源、下载的游戏文件、纹理、语言文件以及其他第三方版权内容不会包含在公开仓库中。

## 构建

### 环境要求

构建 RustCraft 需要：

- 较新的稳定版 Rust 工具链

- Cargo

- 支持 Vulkan 的 GPU

- 最新的显卡驱动

- Vulkan 开发库或 Vulkan SDK

- 合法获取的 Minecraft Java Edition 1.8.9

克隆仓库：

```
git clone https://github.com/YOUR_USERNAME/RustCraft-Public.git
cd RustCraft-Public
```

构建 Release 版本：

```
cargo build --release --locked
```

编译完成后的可执行文件位于：

Windows：

```
target/release/rustcraft.exe
```

Linux：

```
target/release/rustcraft
```

## 准备 Minecraft Assets

RustCraft 需要 Minecraft 原版游戏 Assets 和资源文件才能正常启动和渲染游戏内容。

由于版权和许可原因，这些文件**不会包含在 RustCraft 的公开仓库或 Release 安装包中**。

你需要从自己合法安装的 Minecraft Java Edition 中获取这些资源。

需要准备的 Minecraft 资源主要分为两部分：

1. 由 Minecraft 官方启动器下载和管理的 Assets

2. Minecraft 1.8.9 客户端 JAR 中自带的资源

### 1. 复制官方启动器 Assets

首先，通过 Minecraft 官方启动器安装并至少启动一次 **Minecraft Java Edition 1.8.9**。

这可以确保所需的 Asset Index 和 Object 文件已经被下载到本地。

找到 Minecraft 游戏目录。

Windows：

```
%APPDATA%\.minecraft
```

Linux：

```
~/.minecraft
```

macOS：

```
~/Library/Application Support/minecraft
```

在 Minecraft 目录中找到：

```
assets/
```

该目录通常包含：

```
assets/
├── indexes/
├── objects/
└── ...
```

将这个 `assets` 目录中所需的内容复制到 RustCraft 使用的 `assets` 目录中。

如果使用 Windows 预构建版本，最终目录结构应类似：

```
RustCraft/
├── rustcraft.exe
└── assets/
    ├── indexes/
    ├── objects/
    └── ...
```

如果从源代码运行 RustCraft，则将所需 Assets 放入项目根目录的 `assets` 中：

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
> 请不要从非官方的第三方镜像下载 Minecraft 原版 Assets。请使用你自己的 Minecraft 官方安装中由官方启动器下载的文件。

### 2. 从 Minecraft 1.8.9 JAR 中提取资源

RustCraft 还可能需要 Minecraft 1.8.9 客户端 JAR 内部自带的游戏资源。

首先，请确保已经通过 Minecraft 官方启动器安装 Minecraft Java Edition 1.8.9。

Minecraft 1.8.9 JAR 通常位于：

Windows：

```
%APPDATA%\.minecraft\versions\1.8.9\1.8.9.jar
```

Linux：

```
~/.minecraft/versions/1.8.9/1.8.9.jar
```

macOS：

```
~/Library/Application Support/minecraft/versions/1.8.9/1.8.9.jar
```

JAR 文件本质上基于 ZIP 压缩格式，因此可以使用兼容的压缩软件打开或解压。

请从你自己的 `1.8.9.jar` 中提取 RustCraft 运行所需的 Minecraft 资源目录。

Minecraft 1.8.9 JAR 内的资源通常位于：

```
assets/minecraft/
```

将需要的资源复制到 RustCraft 的 Assets 目录，并保持原有目录结构。

例如，最终目录结构可能类似：

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

随着 RustCraft 的持续开发，实际需要的资源文件范围可能发生变化。

> [!IMPORTANT]  
> 你必须从自己合法获取的 Minecraft 1.8.9 安装中提取这些资源。RustCraft 不提供、不托管，也不重新分发 Minecraft 1.8.9 客户端 JAR 或任何 Minecraft 原版游戏资源。

## 运行

完成所需 Minecraft Assets 的准备后，即可正常启动 RustCraft。

从源代码运行：

```
cargo run --release
```

使用 Windows 预构建版本：

```
rustcraft.exe
```

请确保 RustCraft 能够在运行时找到所需的 `assets` 目录。

如果 RustCraft 无法启动，或出现纹理、模型、声音、语言资源缺失等问题，请检查：

- 是否已经通过官方启动器至少启动过一次 Minecraft Java Edition 1.8.9

- 官方启动器 Assets 是否已经正确复制

- 是否已经从 `1.8.9.jar` 中提取所需资源

- 是否保持了资源原本的目录结构

- RustCraft 是否能够从当前工作目录访问 `assets` 目录

## 资源包

RustCraft 支持 Minecraft 资源包。

资源包 ZIP 文件可以放置在：

```
resourcepacks/
```

资源包可以用于自定义 RustCraft 支持的游戏资源，而无需修改 RustCraft 源代码。

Minecraft 原版资源包以及 Minecraft 原版 Assets 不会作为 RustCraft 仓库的一部分进行分发。

## Shader Packs

> [!WARNING]  
> Shader Pack 支持目前仍处于 **开发中（Work in Progress）**。

RustCraft 正在尝试通过自研的 Vulkan 渲染管线实现 Shader Pack 支持。

目前该功能仍处于实验和开发阶段，尚未完整实现。不保证兼容现有 Minecraft Shader Pack，并且 Shader Pack 系统可能会随着项目开发发生较大变化。

## Lua Modding

RustCraft 包含客户端 Lua 脚本系统，用于为客户端提供灵活的扩展和自定义能力。

脚本系统计划提供受控的游戏 API，包括：

- Camera（相机）

- World（世界）

- Entities（实体）

- Player（玩家）

- Inventory（物品栏）

- Chat（聊天）

Lua API 和 Modding 功能目前仍在持续开发，未来版本中可能发生变化。

## 平台支持

| 平台      | 架构     | 分发格式      |
| ------- | ------ | --------- |
| Windows | x86_64 | ZIP / EXE |
| Linux   | x86_64 | AppImage  |

目前暂未正式支持其他平台和 CPU 架构。

## 下载

预构建的软件包通过 GitHub Actions 自动生成。

开发版本可以通过 GitHub Actions Artifacts 获取。

当推送符合 `v*` 格式的版本 Tag 时，将自动创建对应的版本 Release。

例如：

```
v0.1.0
v0.2.0
v1.0.0
```

Release 软件包**不会包含任何 Minecraft 原版 Assets 或 Minecraft 客户端 JAR**。

用户需要自行从合法获取的 Minecraft 安装中提供所需资源。

## 开发状态

RustCraft 是一个实验性项目，目前尚未达到生产环境可用状态。

当前开发重点包括：

- 提高渲染正确性

- 优化 Vulkan 渲染性能

- 提升 Minecraft 1.8.9 兼容性

- 完善多人游戏网络协议支持

- 提高资源包兼容性

- 扩展 Lua 脚本 API

- 改进跨平台支持

- 开发 Shader Pack 支持

欢迎提交 Bug Report 和代码贡献。

## 法律声明

RustCraft 是一个独立的非官方项目。

RustCraft **与 Mojang Studios 或 Microsoft 没有任何关联，也未获得其认可、赞助或官方支持**。

Minecraft 及其相关名称和资源均属于其各自权利所有者的商标或版权内容。

本仓库不会分发 Minecraft 原版游戏 Assets、Minecraft 客户端 JAR 或其他 Minecraft 专有内容。

用户有责任从自己合法获取的 Minecraft 副本中获取和使用相关资源，并遵守适用的许可协议、服务条款及相关法律。

## 许可证

RustCraft 使用 **RustCraft Noncommercial Source-Available License** 进行授权。

简单来说：

- 允许个人和非商业用途

- 允许查看、学习和修改源代码

- 允许为个人非商业用途进行私有修改

- 未经单独书面许可，禁止商业用途

- 禁止闭源分发修改版本

- 如果向第三方分发修改版本，必须公开相应的完整源代码

- Minecraft 原版资源及其他第三方资源不受 RustCraft 许可证授权

完整许可证条款请参阅 [LICENSE](LICENSE) 文件。
