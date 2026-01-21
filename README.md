# Metal OBJ 渲染示例（Rust）

在 macOS 上使用 Metal 渲染 .obj 模型。

## 说明

- 需要 macOS 支持 Metal。
- 着色器位于 `src/shaders/triangle.metal`。
- 模型请放在 `src/Models/`（优先读取 `src/Models/Bunny.obj`）。
- 使用了 `#![allow(unexpected_cfgs)]` 以规避 `objc` 宏产生的 `cargo-clippy` 警告。

- 需要 macOS 支持 Metal。
- 着色器位于 `src/shaders/triangle.metal`。
