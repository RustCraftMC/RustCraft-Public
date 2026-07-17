# Keystrokes HUD

A configurable keystrokes-style HUD mod for RustCraft.

## Features

- Shows `W`, `A`, `S`, `D`
- Shows `LMB` / `RMB`
- Shows `Sneak`
- Shows `Space`
- Shows combined mouse `CPS`
- Optional `FPS` and ping rows
- Fully movable with anchor + offset controls
- Theme and sizing options

## Controls

Edit the mod config in-game to change:

- Anchor: top-left, top-right, bottom-left, bottom-right
- Offsets: `Offset X`, `Offset Y`
- Scale, key sizes, and text scale
- Box/theme opacity
- Visibility of mouse, sneak, space, CPS, FPS, and ping rows
- CPS counting window

## Notes

- Uses `client.read`, `input.observe`, and `render.custom_draw`
- Reads FPS and latency from `game.client.snapshot()`
- Counts CPS from `attack` and `use` press edges over a sliding time window

