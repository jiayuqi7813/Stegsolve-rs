# StegSolve-rs 
StegSolve-rs 是一个基于 Rust + egui 重构的图像隐写分析工具，复刻重构了StegSolve

> 0.2 版本因跨平台原因全面重构为 egui。
>
> **0.3 版本**：UI 框架升级到 egui 0.33（MSRV 1.88，可在 rustc 1.90+ 上构建；
> 若已 `rustup update` 到 ≥1.92，可在 `Cargo.toml` 改回最新的 egui 0.34 / egui_dock 0.19），
> 并用 `egui_dock` 把原先 5 个独立系统子窗口
> 收进同一个窗口的可停靠标签页（彻底解决子窗口尺寸算不对/显示不全的问题）；
> 同时把全部隐写算法抽成独立、已单测的 `stegsolve-core` crate，并修复了一批对照
> 原版 Java 发现的提取/合成正确性缺陷（见下文"0.3 修复"）。

## 项目结构

```
stegsolve-core/   # 纯算法层（GUI 无关，含 36 个单元/集成测试）
  src/transform.rs   位平面 / 通道掩码 / 反色 / 随机映射 / 灰度
  src/extract.rs     LSB/MSB 数据提取
  src/stereo.rs      立体图
  src/combine.rs     双图合成（13 模式）
src/              # egui 0.34 + egui_dock 单窗口 UI
  app.rs             应用主体 + 各标签页
  frames.rs          GIF/APNG/WebP 帧加载
  fileanalysis.rs    文件格式分析
  apng_decoder.rs    APNG 解码
```

## 主要功能
java原版全功能重构

![image-20250214173053745](/img/image-20250214173053745.png)


## 功能截图

![image-20250214173152024](/img/image-20250214173152024.png)

![image-20250214173208188](/img/image-20250214173208188.png)

## 如何运行
`git clone https://github.com/jiayuqi7813/Stegsolve-rs.git`

`cargo run`

你也可以选择直接下载release版本运行，目前支持windows-x64、linux-x64、linux-aarch64、macos-arm64、macos-x64。需要更多平台可以提交issue。

运行测试：

```shell
cargo test            # 运行 stegsolve-core 的全部单元/集成测试
```

linux 需要如下依赖（0.3 起 rfd 改用 xdg-portal 后端，**不再需要 gtk3**）
```shell
 sudo apt-get install -y libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

## Why Rust+egui?
egui确实文明，gtk坑太多了（——0.3 起连 gtk 运行时依赖也一并去掉了。

## MCP 服务（供大模型调用）

内置一个本地 MCP 服务，让任意支持 MCP 的大模型客户端（Claude Desktop / Cline 等）
直接调用本工具的隐写分析能力。

用法：打开软件 → 菜单「工具 → MCP 服务」→ 点「启动」→ 把显示的 URL（如
`http://127.0.0.1:7878/mcp`）和令牌加到你的 MCP 客户端即可。

提供的工具：`analyze_format`（文件格式）、`list_transforms`、`render_transform`、
`bit_plane`（位平面，**返回图片让模型直接看**）、`extract_data`（LSB/MSB 提取）、
`auto_lsb`（自动尝试常见配置、找 flag）、`combine_images`、`stereo`、
`frame_count` / `extract_frame`。

实现：官方 Rust SDK `rmcp` 的 Streamable-HTTP 传输，GUI 进程内托管；
绑定 `127.0.0.1`，默认开启 Bearer 令牌鉴权；工具逻辑复用 `stegsolve-core`，
含 7 项 MCP 测试（含真实网络层 + 令牌校验）。

## 0.3 修复（对照原版 Java StegSolve）

- **数据提取 · LSB-First 多通道顺序**：旧实现 LSB 优先时通道顺序写反（红蓝对调），
  多通道 LSB 提取结果错误。已修复，并用"嵌入→还原"集成测试固化。
- **图像合成器 · 6 个模式缺失**：SUB / SUB(分通道) / MUL / MUL(分通道) / Lightest /
  Darkest 此前全部退化为"显示图1"，现已全部按原版补齐。
- **变换 · Full alpha**：此前恒为纯红、丢失真实透明度，现正确读取 alpha 通道。
- **变换 · 索引调色板随机映射**：补充 distinct-color 随机置换，可揭示调色板隐写。
- 预览控制符不再破坏 hexdump 对齐；另存为无扩展名默认 `.png`；移除立体图调试输出。

## 已知问题

- [x] 子 UI 页面大小计算有问题、部分页面显示不全 —— **0.3 已通过单窗口 Dock 彻底解决**

## todo

- [x] 自动流水线打包全平台
    - [x] 全平台预编译打包支持
- [x] 项目结构重构（0.3：core/UI 分层 + 单测）
- [ ] 多语言支持

## 贡献
感谢以下贡献者的贡献

[@b3nguang](https://github.com/b3nguang) 

## 许可证
[MIT License](LICENSE)
