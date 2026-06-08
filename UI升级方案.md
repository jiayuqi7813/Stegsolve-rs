# StegSolve-rs UI 框架升级方案

> 版本：v1（对比决策稿） · 日期：2026-06-08
> 代码基线：`main` @ `f07f854`，当前 UI 栈为 `eframe`/`egui 0.31`

---

## 1. 现状与核心问题

项目是 Java 版 StegSolve 的 Rust 重构，纯 Rust + egui（immediate mode）。源码约 2900 行，分布如下：

| 文件 | 行数 | 职责 |
|---|---|---|
| `main.rs` | 538 | 应用入口、菜单栏、主图显示、底部控制条、5 个子窗口的弹出逻辑 |
| `fileanalysis.rs` | 598 | 文件格式 / PNG / APNG chunk 分析报告 |
| `extractanlysis.rs` | 506 | 数据提取对话框（位平面、通道、行列、MSB/LSB） |
| `transform.rs` | 362 | 41 种通道/位平面变换算法 |
| `combine.rs` | 284 | 双图合成器（13 种叠加模式） |
| `apng_decoder.rs` | 205 | APNG 手写解码 |
| `framebrowser.rs` | 191 | GIF/APNG/WebP 帧浏览 |
| `stereo.rs` | 190 | 立体图（XOR 偏移）分析 |

### 当前 UI 架构的问题（按严重度排序）

1. **多视口（multi-viewport）滥用是最大痛点。** 5 个分析功能（文件分析、数据提取、立体图、帧浏览器、图像合成）每个都用 `ctx.show_viewport_immediate` 开独立 OS 窗口。代码里每个窗口都重复同一套 `viewport_id + should_close + send_viewport_cmd(Close)` 样板（`main.rs` 244–437 行），逻辑高度重复、难维护。README 自己也记录了"子 UI 页面大小计算有问题，部分页面显示不全""有些界面会突然很大"——这正是 immediate-mode 多视口在不同平台/DPI 下尺寸不可控的典型表现。

2. **`FileAnalysis::new` 在每帧重建。** `main.rs` 264 行每帧 `FileAnalysis::new(file_path)` 都重新读盘+完整分析一次文件，纯属浪费（immediate mode 下 update 每秒跑 60 次）。

3. **状态管理散乱。** 一个 `StegApp` 平铺了 `show_*: bool` + `Option<Dialog>` 两套并行状态，开关与实例生命周期分离；`combine.rs` 用 `Rc<RefCell<>>` 绕 immediate-mode 的借用约束，是 workaround 而非干净设计。

4. **版本落后 3 个小版本。** 当前 `egui 0.31`（2025 初），最新已是 **`egui 0.34.2`（2026-05-04）**。期间 0.32 引入了 atoms 组件系统、0.33/0.34 有大量布局与渲染改进。

5. **零测试、零关注点分离。** 算法（transform/stereo/combine）和 UI 渲染耦合在同一文件里，无法单测算法正确性。

6. **交互细节粗糙。** 缩放靠"鼠标在窗口内就把滚轮全吃掉"（`main.rs` 457 行），导致滚动和缩放冲突；导航按钮高亮靠手动判键。

---

## 2. 三套升级方案对比

下面三条路线，从"改动最小最稳"到"UI 能力最强"排列。

### 方案 A —— 留在 egui，升级到 0.34 + egui_dock 深度重构（推荐基线）

把 egui 升到 0.34.2，**用 `egui_dock 0.19` 的 Tab/Dock 系统把 5 个外部窗口收进单窗口的可停靠标签页**，彻底干掉 multi-viewport 那套样板和尺寸 bug。同时做关注点分离：算法层（transform/stereo/combine/分析）抽成纯函数模块并加单测，UI 层只负责渲染。

- **工作量**：中（约 3–5 天）。算法零改动，主要重写 `main.rs` 的窗口管理 + 各 dialog 的 `ui()` 签名统一为 dock tab。
- **风险**：低。纯 Rust、技术栈不变、依赖成熟，编译/打包流程（现有 GitHub Actions 全平台）几乎不动。
- **收益**：直接消灭 README 记录的尺寸 bug；单窗口多标签体验远好于一堆乱飞的子窗口；代码可测、可维护。
- **产物体积**：基本不变（单个原生二进制，约 10–20MB）。
- **迁移路径**：渐进式，可一个 dialog 一个 dialog 迁移，每步都能编译运行。

### 方案 B —— 换 Tauri 2 + Web 前端（UI 能力最强，工程量最大）

Rust 隐写算法层保留为后端（通过 `#[tauri::command]` 暴露），UI 全部用 Web 技术重写（React / Svelte / SolidJS + Tailwind）。**Tauri 2 已于 2024-10 稳定，生产级**，被 Spacedrive、AppFlowy 等采用，还支持 iOS/Android。

- **工作量**：大（约 2–4 周）。要新建前端工程、设计前后端 IPC 协议、把图像数据在 Rust↔WebView 间传输（大图需走 base64/二进制流，有性能与内存设计成本）。
- **风险**：中高。引入 Node/前端工具链；图像逐像素变换走 IPC 到 WebView 渲染需谨慎设计，否则大图卡顿；团队需要前端技能。
- **收益**：UI 上限最高——动画、主题、响应式布局、复杂交互随便做；天然支持后续做 Web 版 / 移动版。
- **产物体积**：比纯原生略大但仍远小于 Electron（Tauri 用系统 WebView，通常几 MB~十几 MB）。
- **迁移路径**：算法可复用，但 UI 是**推倒重写**，不是渐进迁移。

### 方案 C —— 换 Slint（声明式、纯 Rust 友好、原生性能）

换成 **Slint 1.16**（2026-04）。Slint 用 `.slint` 声明式标记语言写 UI + 响应式属性系统，运行时 <300KiB RAM，原生渲染（非 WebView），对 Rust 一等公民。适合这类"工具型、布局相对固定"的桌面应用。

- **工作量**：中大（约 1.5–3 周）。要学 `.slint` DSL，把所有界面用声明式重写，并设计 Rust 后端与 `.slint` 的属性/回调桥接。
- **风险**：中。生态比 egui/Tauri 小，复杂自定义控件（如可缩放拖拽的大图画布、逐像素 overlay）需要验证；社区资料较少。
- **收益**：声明式布局比 immediate-mode 更适合做稳定的工具界面，能彻底解决尺寸计算问题；原生性能、低内存；UI 与逻辑分离干净。
- **产物体积**：小（原生二进制，和 egui 相近）。
- **迁移路径**：算法可复用，UI 重写，但比 Tauri 工程量小、无需前端工具链。

### 横向对比表

| 维度 | A. egui 0.34 + dock | B. Tauri 2 + Web | C. Slint 1.16 |
|---|---|---|---|
| 工作量 | ★★☆☆☆ 中 | ★★★★★ 大 | ★★★★☆ 中大 |
| 风险 | 低 | 中高 | 中 |
| 纯 Rust | ✅ 是 | ❌ 需前端栈 | ✅ 是（+DSL） |
| UI 能力上限 | 中 | 最高 | 中高 |
| 解决现有尺寸 bug | ✅ 彻底 | ✅ 彻底 | ✅ 彻底 |
| 大图逐像素渲染 | ✅ 原生纹理，强 | ⚠️ 需设计 IPC | ✅ 原生 |
| 产物体积 | 小 | 中 | 小 |
| 打包流程改动 | 几乎不动 | 大改 | 中改 |
| 跨平台现状 | 已支持 5 平台 | +移动端 | +移动端 |
| 迁移方式 | 渐进式 | 推倒重写 UI | 重写 UI |
| 学习成本 | 无（同栈） | 前端 + Tauri | Slint DSL |

---

## 3. 推荐

**首选方案 A（egui 0.34 + egui_dock 深度重构）。** 理由：

- 你的核心痛点是"UI 框架存在很大问题"，而经分析，**真正的问题不是 egui 本身，而是 multi-viewport 的误用 + 版本落后 + 缺少分层**。方案 A 用单窗口 dock 标签页直接治本，且改动最小、风险最低、可渐进验证。
- 项目定位是 CTF/隐写分析工具，用户看重的是功能正确和稳定，不是花哨 UI——egui 的能力完全够用。
- 保持纯 Rust 单二进制，现有全平台 CI/打包不用重做。

**如果你的真实目标包含"未来要做 Web 版或移动端 / 要做现代化炫酷 UI"**，那值得直接上方案 B（Tauri 2），一步到位，但要接受 2–4 周工程量和前端栈引入。

方案 C 适合"想要纯 Rust + 声明式、但又嫌 Tauri 太重"的折中，技术上可行，只是生态和资料偏少，对这个项目收益不如 A 明显。

---

## 4. 方案 A 分阶段实施计划（推荐落地路径）

> 全程渐进式，每个阶段结束都可编译运行，不破坏现有功能。

**阶段 0 · 准备（0.5 天）**
- 升级 `Cargo.toml`：`eframe`/`egui` → `0.34`，新增 `egui_dock = "0.19"`，同步 `image`/`rfd` 等到兼容版本。
- 跑通编译，修复 0.31→0.34 的 API 破坏性变更（atoms、`menu::bar`、纹理 API 等）。

**阶段 1 · 算法层分离 + 正确性修正 + 单测（2 天）**
- 把 `transform.rs`、`stereo.rs`、`combine.rs`、`extractanlysis.rs` 的纯算法与 UI 解耦，整理成 `core/` 模块。
- **修复第 5 节列出的全部隐写/提取正确性缺陷**（数据提取 LSB 顺序、合成器缺失模式、Full alpha 等）。
- 以原版 Java 的输出为基准，为 41 种变换、立体偏移、13 种合成模式、数据提取各分支补单测（既保证重构不改变正确像素输出，又锁定 bug 修复结果）。

**阶段 2 · 单窗口 Dock 化（1.5 天）**
- 引入 `egui_dock::DockState`，把文件分析、数据提取、立体图、帧浏览器、图像合成 5 个功能从 `show_viewport_immediate` 改成 dock 内的 Tab。
- 删除全部 `viewport_id / should_close / send_viewport_cmd` 样板，统一 `show_*: bool` 与 `Option<Dialog>` 为一套 tab 注册表。

**阶段 3 · 主图画布与交互重做（1 天）**
- 主图查看器抽成独立组件：修正缩放/滚动冲突（缩放只在 Ctrl+滚轮或专用区域生效），加适配窗口、1:1、缩放百分比显示。
- 修掉 `FileAnalysis::new` 每帧重建——改为打开时构建一次、缓存结果。

**阶段 4 · 收尾（0.5 天）**
- 统一主题/字体（保留 MiSans），暗色模式开关。
- 更新 README，跑通 CI，全平台打包验证。

**总计约 5–6 天**（含算法修正）。

---

## 5. 隐写算法与数据提取的正确性缺陷（对照原版 Java StegSolve）

> 已逐行对照原版 `souno-io/Stegsolve`（1.5 修复版）的 `Transform.java`、`Extract.java`、`CombineTransform.java`、`StereoTransform.java`。
> **这些是算法 bug 与功能缺失，与选 A/B/C 哪套 UI 无关——任何方案都必须修。**

### 5.1 数据提取（`extractanlysis.rs`）

| # | 严重度 | 问题 | 原版 Java 行为 | 当前 Rust 行为 | 修复 |
|---|---|---|---|---|---|
| E1 | **高** | **LSB-First 多通道顺序反了（红蓝互换）** | MSB/LSB 两条路径里 R/G/B 顺序一致 | `extract_bits` 的 `lsb_first` 分支里 `channels` 数组按字节位置写反了：标"RGB"实际按 **B,G,R** 提取（绿居中不变，红蓝对调）。**只要勾选 ≥2 个颜色通道且选 LSB-First，提取出的字节流就是错的**（单通道不受影响） | 把 LSB 分支六个数组改成与 MSB 分支镜像：RGB 应为 `[1<<24,1<<16,1<<8]` 等 |
| E2 | 低 | 预览 ASCII 列被控制符破坏 | 仅 32–128 原样输出，其余显示 `.` | 用 `is_ascii_graphic() \|\| is_ascii_whitespace()`，导致 `\n \t \r` 被原样打印，破坏 hexdump 对齐 | 仅 `0x20..=0x7E` 原样，其余 `.` |

> 说明：E1 是这次"数据提取不正确"的主因。MSB-First 路径是对的；行/列、掩码、bit 拼接逻辑都与原版一致。

### 5.2 隐写变换（`transform.rs`）

| # | 严重度 | 问题 | 原版 Java 行为 | 当前 Rust 行为 | 修复 |
|---|---|---|---|---|---|
| T1 | 中 | **"Full alpha" 视图失效** | `transmask` 读真实 alpha（`getRGB`），把 alpha 通道当红色梯度显示 | `transform_mask` 把 alpha 硬编码成 255，导致 Full alpha 永远是纯红，丢失真实透明度信息（注：单个 alpha 位平面 `transform_bit` 读的是真实 alpha，是对的，只有这个合成视图坏了） | `transform_mask` 的 `fcol` 用真实 `pixel[3]`，与 `transform_bit` 一致 |
| T2 | 中 | **索引/调色板随机映射缺失** | `random_indexmap` 对**调色板**随机重映射，能揭示 GIF/索引 PNG 的调色板隐写（两个索引同色的情况） | `random_indexmap`/`random_map` 一律退化成真彩色随机，索引图丢失该能力（因 `image` 解码即转 RGBA） | 检测索引 PNG/GIF，保留调色板并对调色板做随机重映射 |
| T3 | 低 | 随机颜色映射公式与原版不符 | `b=((blue*bm)*bm ^ bx)+ba`（系数乘两次）、保留 alpha、通道位置不变 | 只乘一次、且把输入 R 写到输出 B（红蓝错位） | 按原版公式对齐（因随机，影响小，属保真问题） |

### 5.3 图像合成器（`combine.rs`）

| # | 严重度 | 问题 | 原版 Java | 当前 Rust | 修复 |
|---|---|---|---|---|---|
| C1 | **高** | **13 个合成模式里有 6 个没实现** | 0–12 全部实现 | `combine_pixels` 的 `match` 只写了 0–4（XOR/OR/AND/ADD/ADD分通道），**5–10（SUB、SUB分通道、MUL、MUL分通道、Lightest、Darkest）全部落到 `_ => 显示图1`**，但界面标签照常显示"SUB""MUL"等 → 一半模式是假的 | 按 Java `comb()` 补齐 case 5–10 |
| C2 | 低 | ADD 语义与原版不同 | case 3 `ADD` = 整型 `c1+c2`（通道间进位会串色） | 用逐通道 `saturating_add`（钳位、不串色） | 决定保留哪种语义并与标签一致 |

### 5.4 立体视图（`stereo.rs`）

逻辑**正确**，与 `StereoTransform.java` 完全一致（XOR 水平偏移、按宽度回绕）。唯一小问题：`back()/forward()` 里残留 `println!` 调试输出，应删除。

### 5.5 其他对照发现（次要）

- **另存为无扩展名会崩**：原版默认存成 `.bmp`，Rust 直接 `img.save(path)` 在无扩展名时报错。建议无扩展名时默认 `.png`。
- 随机映射用了 `rand 0.9` 的旧 API（`thread_rng`/`gen_range`），有弃用警告，顺手迁到 `rng()`/`random_range`。

> 优先级建议：**E1、C1 先修**（直接影响解题正确性），其次 T1、T2，再到其余保真项。这些修复会随阶段 1 的"算法层分离"一并落地，并用对照原版的单测固化。

---

## 6. 待你确认

1. **选哪条路线**：A（推荐）/ B（Tauri 现代化）/ C（Slint）？
2. 若选 A，是否同意按上面的阶段渐进重构（我会在每阶段产出可运行代码）？
3. 是否有"未来要上 Web/移动端"的硬需求？有的话我会重新权衡是否直接走 B。
4. 第 5 节的算法/提取修复要不要**先行单独做掉**（不依赖 UI 重构，可立刻动手），还是统一并入阶段 1？我建议 E1、C1 这两个高危项先修。

---

### 参考来源（原版 Java 对照）
- 原项目：https://github.com/souno-io/Stegsolve （1.5 修复版）
- `Transform.java` / `Extract.java` / `CombineTransform.java` / `StereoTransform.java`（`stegsolve/` 目录）

---

### 参考来源
- egui 最新版 0.34.2（2026-05-04）— https://github.com/emilk/egui/releases
- egui_dock 0.19.1（支持 egui 0.34）— https://docs.rs/egui_dock/latest/egui_dock/
- Tauri 2.0 稳定版（2024-10）— https://v2.tauri.app/blog/tauri-20/
- Slint 1.16（2026-04）— https://slint.dev/blog/slint-1.15-released
