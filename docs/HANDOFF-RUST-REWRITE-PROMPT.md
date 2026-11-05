# 交接提示词

当前接续：2026-11-05用户已批准独立公开行为实现、个人使用、可用SuperVod JSON首版及文档/基线迁移；先读HANDOFF“当前接续更新”、ADR-048和最新PROGRESS/task status。下面为迁移前原提示词，首次方案确认门已获用户答复；当前仍须完成原rebaseline和新基线独立文档CLEAR后开发，逐卡验收/verify/提交/独立巡检保持。AES已静态核实普通JSON无需新增插件解密，实际媒体闭环NOT RUN。

把下面分隔线之间的内容完整发给 Codex。工作目录设为 `/Users/eason/jp/application`。

---

你接手项目“映象视频”，一个 Tauri 2 + Rust + Vue 3 的桌面影视客户端。工作目录是 `/Users/eason/jp/application`。

**用户刚决定放弃 Android 兼容层，改用 Rust 重写播放源。规格和任务卡尚未修订，重写尚未开始。**

先完整阅读，再行动：

1. `docs/HANDOFF-RUST-REWRITE.md`，这是当前状态、决定和待办的权威说明。
2. `AGENTS.md`，项目规则和回答句式限制都在这里，必须遵守。
3. `docs/DECISIONS.md` 的 ADR-043 至 ADR-047。
4. `docs/PROGRESS.md` 末尾几条和 `docs/BACKLOG.md`。

然后运行 `.venv/bin/python scripts/task.py check` 和 `.venv/bin/python scripts/task.py status`，核对现状与交接文档一致。如有出入，以实际情况为准并告诉用户。

## 你的第一个任务：只做方案，不写实现

产出一份简短的修订方案交给用户确认，内容是：

1. 交接文档第 6 节的每个待决事项，列出选项和你的建议。**其中许可和内容版权必须由用户决定，不能替用户默认。** 插件源码所在仓库未见许可证，照源码翻译的风险要明确说明。
2. T0.19 至 T0.22 改成哪些 Rust 播放源卡，T0.23 至 T0.29 和 M 系列怎么调整，每张卡的范围和验收标准。
3. 建议的 Rust 结构：优先复用 `maccms.rs`、`source_dispatch.rs`、`resolution.rs`、`gateway.rs`。遵守奥卡姆原则，只有在确实需要时才新增 provider 抽象。
4. 对每个站点先做只读可达性核实，把已经不能访问的站点标出来。这一步只发普通请求，不做规避。

**在用户回复确认前，不要修改 `docs/TASKS.md`、`docs/ARCHITECTURE.md`、`scripts/` 或产品代码，也不要执行 `rebaseline`。**

## 确认之后

按交接文档第 7 节修订文档并迁移基线，再按第 8 节逐卡实现，严格按任务顺序。建议先做 SuperVod，因为它最可能直接复用 MacCMS 适配器；这一点尚未验证，先核实它的 AES 用途。

## 必须遵守的底线

- 不伪造验收、授权或用户决定；未运行的写 NOT RUN；零测试不算通过。
- 验收标准按 ADR-044 的娱乐软件定位，不要重新加严；但安全、鉴权、数据完整性和功能正确性不放宽。
- 不删除失败历史；`docs/PROGRESS.md`、`docs/DECISIONS.md`、`docs/BACKLOG.md` 只追加。
- 不要自行 `drive.py accept-review`；巡检 `BLOCKING` 要修复。确认的产品缺陷直接修，证据类细项自行判断并如实记录。
- 不绕过验证码、Cloudflare 或站点的访问控制。遇到这类情况，停下来报告给用户。
- 凭据和带 capability 的 URL 不进代码、日志或报告。
- Git 不推送、不改写历史。提交信息格式见 `AGENTS.md`。
- 权限受限时准确说明并让用户处理，不换写法绕过。
- 需要打包窗口的验收要求屏幕解锁；锁定时告诉用户，不要改系统设置。

## 沟通

用中文，直接说结论。只有用户才能做的事才打断用户：决定许可和范围、解锁屏幕、提供凭据。其余按任务顺序继续推进。每次结束时按 Changed / Why / Verified / Remaining 报告，如实写出未验证的内容。

---
