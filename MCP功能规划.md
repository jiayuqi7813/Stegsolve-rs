# StegSolve-rs · MCP 功能规划

> 目标：让大模型（Claude Desktop / Cline / Cherry Studio 等任意支持 MCP 的客户端）能直接调用
> StegSolve-rs 的隐写分析能力。用户体验就是一句话：**打开软件 → 点一下"启动 MCP 服务" → 大模型就能用了。**

---

## 1. 用户故事

1. 用户双击打开 StegSolve-rs。
2. 菜单/面板里有个「MCP 服务」开关，点「启动」。
3. 软件在本机起一个 MCP 服务，显示一行地址，例如 `http://127.0.0.1:7878/mcp`（带一个一键复制按钮和一个随机访问令牌）。
4. 用户把这个地址（+令牌）粘到自己的大模型客户端的"自定义连接器/MCP Server"里。
5. 之后就能直接对大模型说："分析这张图有没有 LSB 隐写"，模型会自己调用工具：看文件格式、逐个看位平面、跑 LSB 提取、把可疑字符串/flag 提取出来。
6. 用完点「停止」。

这个体验只有 **Streamable HTTP** 传输能满足（见下一节）。

---

## 2. 传输方式选型：为什么是 Streamable HTTP

MCP 有两种主流传输：

| 传输 | 谁来启动服务 | 是否符合"点按钮启动" | 适用 |
|---|---|---|---|
| **stdio** | 由大模型客户端把服务当子进程拉起 | ❌ 不符合——不是用户点按钮，是客户端拉起的，且 GUI 程序不适合当 stdio 服务 | 命令行式本地工具 |
| **Streamable HTTP** | 服务自己监听一个端口，客户端连 URL | ✅ 完全符合——GUI 进程内托管，按钮启停，给出 URL | 本地常驻/可视化工具、远程服务 |

**结论：主线方案 = GUI 进程内托管 Streamable HTTP MCP 服务**，绑定 `127.0.0.1`，按钮启停。
（可选附带一个 stdio 独立 bin，给偏好 stdio 的高级用户，工具集完全复用，零额外逻辑。）

技术依据：官方 Rust SDK [`rmcp` 1.7](https://crates.io/crates/rmcp) 提供 `transport-streamable-http-server` 特性（基于 axum/tower），可直接在 tokio runtime 里跑 HTTP MCP 服务。

---

## 3. 总体架构

```
┌─────────────────────────────────────────────┐
│ StegSolve-rs 进程                              │
│                                               │
│  egui 主线程（UI）                             │
│    └─ 「MCP 服务」面板：启动/停止、端口、URL、    │
│        令牌、请求日志                           │
│         │ 启动时 spawn                         │
│         ▼                                      │
│  后台线程：tokio runtime                        │
│    └─ rmcp Streamable-HTTP Server (axum)       │
│         └─ Tools ──► 调用 stegsolve-core 算法    │
│                       （纯函数，无共享可变状态）   │
└─────────────────────────────────────────────┘
```

- **复用已分层的 `stegsolve-core`**：所有工具就是 core 算法的薄封装——这正是之前把算法独立成 crate 的红利，MCP 层几乎不含业务逻辑、天然可测。
- **线程模型**：egui 跑主线程；点「启动」时在后台线程建一个 tokio runtime 跑 HTTP 服务；保存一个关闭句柄（`tokio::sync::Notify` / axum graceful-shutdown）用于「停止」和退出时清理。
- **共享状态最小化**：工具只接收输入（文件路径/参数）→ 调 core → 返回结果，不依赖 GUI 当前打开的图。GUI 与服务之间只共享：`running` 标志、端口、URL、令牌、一个 `Arc<Mutex<Vec<日志行>>>`（服务追加、UI 展示）。

---

## 4. MCP 工具清单

> 关键亮点：MCP 工具结果支持 **图片内容块**（base64 PNG）。所以模型不仅能拿到文本，
> 还能"亲眼看到"某个位平面/合成图，再决定下一步——这对隐写分析价值极大。
> rmcp 用 `Content::image(...)` 返回图片。

| 工具 | 入参 | 返回 | 调用的 core |
|---|---|---|---|
| `analyze_format` | `path` | 文本：文件格式报告（BMP/PNG/GIF/JPEG、chunk、APNG 等） | `fileanalysis::analyse_file_format` |
| `list_transforms` | — | 文本：0–41 号变换名称表（帮模型挑） | `transform::get_text` |
| `render_transform` | `path`, `trans`(0–41), `save_path?` | **图片**(PNG) + 文本说明 | `Transform` |
| `bit_plane` | `path`, `channel`(r/g/b/a), `plane`(0–7), `save_path?` | **图片**(PNG)（内部映射到对应 trans 号） | `Transform` |
| `extract_data` | `path`, `red_bits[]`,`green_bits[]`,`blue_bits[]`,`alpha_bits[]`, `bit_order`(msb/lsb), `direction`(row/col), `rgb_order`(rgb…bgr), `max_preview`? , `save_path?` | 文本：hex+ascii 预览；base64：原始字节 | `extract::generate_extract/preview` |
| `auto_lsb` | `path`, `min_len`? | 文本：自动尝试一组常见配置（RGB-LSB 行/列、各单通道 LSB、MSB/LSB），用类 strings 过滤后给出可疑可打印串/flag 候选 | `extract::*` 组合 |
| `combine` | `path1`, `path2`, `mode`(0–12 或名称), `save_path?` | **图片**(PNG) | `combine::combine` |
| `stereo` | `path`, `offset`, `save_path?` | **图片**(PNG) | `stereo::StereoTransform` |
| `frame_count` | `path` | 文本：帧数 | `frames::load_frames` |
| `extract_frame` | `path`, `index`, `save_path?` | **图片**(PNG) | `frames::load_frames` |

- 入参用本地文件路径（本地工具最省事）；可选 `save_path` 让模型把结果落盘。
- 大图返回前可选下采样，避免 base64 体积过大（设上限，如最长边 1024）。
- `auto_lsb` 是给"模型自动解题"准备的高价值便捷工具，一次调用覆盖最常见的 LSB 套路。

---

## 5. GUI 集成（「MCP 服务」面板）

新增一个 dock 标签页或菜单项「MCP 服务」，内容：

- **状态灯**：未启动（灰）/ 运行中（绿，显示已处理请求数）。
- **启动 / 停止** 按钮。
- **端口**输入（默认 7878）。
- **URL**：`http://127.0.0.1:<port>/mcp` + 「复制」按钮。
- **访问令牌**：启动时随机生成，客户端需在 `Authorization: Bearer <token>` 带上；「复制」按钮。
- **请求日志**：滚动显示最近的工具调用（工具名、入参摘要、耗时、成败）。
- **一键接入提示**：展示如何在 Claude Desktop / Cline 等添加该 URL。

启停逻辑：点「启动」→ 后台线程建 tokio runtime + 起 axum 服务；点「停止」或关闭软件 → 触发 graceful shutdown，join 线程。

---

## 6. 依赖与构建影响（仅 bin，core 不动）

```toml
# rmcp：HTTP 服务端 + 宏 + schema
rmcp = { version = "1.7", features = [
  "server", "macros", "schemars", "base64",
  "transport-streamable-http-server",
] }
tokio = { version = "1", features = ["rt-multi-thread", "sync", "macros"] }
# axum/tower 由 rmcp 的 http-server 特性带入；另需 base64、serde/serde_json（rmcp 自带）
```

- 全部纯 Rust，**不引入任何系统库**——可在当前 CI / 各平台正常构建（和 GPUI 不同，这条能在本地沙箱里 `cargo build` 验证）。
- 体积增加有限（tokio + axum），可接受。
- 可选：再加一个 `[[bin]] stegsolve-mcp`（stdio 版），features 用 `transport-io`，复用同一套工具定义。

---

## 7. 安全

- 默认只绑 `127.0.0.1`（不对外网暴露）。
- 启动时生成随机 Bearer 令牌，服务校验；UI 展示供复制。
- 工具只读图片/可选写指定 `save_path`；不暴露任意文件读写、不执行 shell。
- 文档里提示：这是本机服务，端口仅本机可达；如需更强隔离可加 IP 白名单。

---

## 8. 分阶段实施

**P0 · 工具层（0.5–1 天）**
- 在 bin 里新增 `mcp/tools.rs`，把上面 10 个工具写成 rmcp `#[tool]`，全部转调 `stegsolve-core` / 现有 `fileanalysis`/`frames`。
- 先挂 stdio（`transport-io`），用官方 `@modelcontextprotocol/inspector` 本地连通、逐个验证工具。

**P1 · HTTP 服务 + GUI 启停（1 天）**
- 接 `transport-streamable-http-server`，后台 tokio 线程托管。
- 新增「MCP 服务」面板：启动/停止、端口、URL、令牌、日志。

**P2 · 图片工具 + auto_lsb（0.5–1 天）**
- 位平面/合成/立体/帧等工具返回 PNG 图片内容（含下采样上限）。
- 实现 `auto_lsb` 便捷工具。

**P3 · 打磨（0.5 天）**
- 令牌校验、端口配置持久化、复制按钮、退出时优雅关闭、错误提示。

**合计约 2.5–3.5 天。**

---

## 9. 测试与验证

- core 算法已有 36 个测试，工具是薄封装，可直接对工具函数加少量集成测试。
- 用 `npx @modelcontextprotocol/inspector` 连本地服务，逐工具点测（看 schema、调用、图片返回）。
- 端到端：在 Claude Desktop 里加该 URL，用一张藏了 flag 的图验证模型能 `auto_lsb` 出 flag。
- 这套（rmcp/tokio/axum 纯 Rust）可在当前构建环境 `cargo build` 通过——实现时我能像 egui 那轮一样给你编译验证。

---

## 10. 风险与开放问题

- **egui 主线程 + tokio 后台**：注意句柄管理与退出清理，避免端口泄漏 / 线程悬挂。
- **rmcp 1.x API**：1.7 已较稳定，但实现时以当时版本的 `#[tool]`/`ServerHandler` 实际签名为准。
- **图片体积**：大图 base64 可能撑大上下文，需下采样 + 上限。
- **客户端兼容**：Claude Desktop 自定义连接器支持远程 HTTP MCP（带鉴权）；个别客户端可能只支持 stdio——故保留可选 stdio bin。
- **待你确认**：默认端口（7878？）、是否要令牌鉴权（建议要）、是否同时提供 stdio bin。

---

### 一句话总结
复用已经独立且测过的 `stegsolve-core`，用 rmcp 的 Streamable-HTTP 在 GUI 进程里托管一个本地 MCP 服务，
一个按钮启停、给出 URL，大模型连上即可"看位平面、跑 LSB、自动找 flag"。纯 Rust、可本地编译验证、不破坏现有跨平台打包。
