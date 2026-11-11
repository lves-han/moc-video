# 文档审查报告 round-51：T0.36 生命周期判据复审（ADR-058）

## 受审基线

- Git HEAD：`78dee2837ba6e162bd63ec69983447d46d1a2769`，工作树干净。修订范围 `dc49b61..78dee28` 涉及 TASKS.md、DECISIONS.md 和 round-50 报告。
- `task.py status` 的 protected：`d26c7b3c0e7bd447cbf049df27b05877ea25b037e5d3c8f33738751934e9dfd5`。
- `state.protected`：`d441fca11eb084cd9b4e82156ff1e3e1c867de608b66187879689ff03d07572a`（待 rebaseline，属协调方说明的预期状态）。
- ADR 分段摘要：ADR-058（含补充）`0b5d21df283072c98822e489c7e530a1250ccc25f7eb396999be40d1d3fba5c3`；ADR-057 `5b8dae50baade73ea4b9f6217ef21d8f42636df4f4a4512ca75fc478d1898d1a`（未变）。
- current=T0.27，blocked=`T0.27`。`task.py check`：`roadmap OK: 42 cards`，退出 0。

## 结论

round-50 的 B1 已解决，修订未引入新矛盾，无阻断问题。规划足以进入下一步，业务验收全部 NOT RUN。

| round-50 B1 要求 | 修订位置 | 判定 |
| --- | --- | --- |
| 数据库版本 1→2 | TASKS.md T0.36：`upgrade_before` 1，`upgrade_after`、`uninstalled` 2 | 已解决，与 `lifecycle()` 的三个检查阶段和 C17 一致 |
| 设置比较 | 排除 `metadata_preset_applied`，升级前配置须已含 Stremio，其余字段逐项相等，`watch_history` 逐行相等 | 已解决，强度未降低；`uninstalled` 与 `upgrade_after` 的相等比较不受影响 |
| manifest、构建收据、资源归属 | `release.py` 的 `card`、`artifact()` 校验、构建收据和资源归属统一为 T0.36 | 已解决，覆盖 `release.py:316`、`artifact()` 和 `t10.resources` 的卡号 |
| 时机 | “在 `release.py --build` 和人工观察之前完成” | 已解决，避免人工观察作废 |
| `test_t23.py` 同步 | 同步正负例 | 已解决 |
| 判据性质说明 | T0.36 工作义务和 ADR-058 补充，明确干净系统、默认安全、quarantine、观察绑定、声音确认强度不变 | 已解决 |

范围核对：T0.36 范围含 `packaging/`（`release.py`、`T23.md`）和 `tests/`（`t23.py`、`test_t23.py`），覆盖全部待改文件。

## 其他核实

- T0.27 新增义务与 A1 一致：A1 使用 `--suite package`，`DEFERRED.reproduce_argv` 改为同一 suite，移除 `--suite all` 不影响 A1；`limitations` 写明生命周期由 T0.36/A2 执行，与 A1 描述和 ADR-058 一致。`packaging/T23.md` 同步义务覆盖其中的旧命令。
- 覆盖表 U04、REQ-T23 行的“包级”标注准确，`task.py check` 仍通过。
- 证据保留义务和提前通知用户的提醒已加入 T0.36，符合 AGENTS.md。
- `A2` 的 argv、cases 和预算（19440 秒 ≤ 21600 秒）未变。
- round-50 的 N1、N2、N4、N5 已采纳；N3 与 S12 未处理，均为非阻断。

## 待执行前提（不判 BLOCKING）

CLEAR 后由用户授权，RESOLVED 决定同时覆盖 round-48 修订和 ADR-058（含本次补充），批准摘要须等于当时的 protected（当前 `d26c7b3c…`），再执行 `task.py rebaseline T0.27 --protected <同一摘要>`。最新 BLOCKED 记录已存在，追加 RESOLVED 后控制器条件满足，顺序合理。此后 T0.27 按 `--suite package` 完整重验，无需 UTM。

## 建议（非阻断）

| ID | 位置 | 建议 |
| --- | --- | --- |
| N1 | T0.27 的“报告标明实际 suite” | `tests/acceptance/t14.py:158` 写死 `suite="all"`，JUnit 名也带 `-all`。优先在 t23 的 `Acceptance` 子类覆盖，避免改动其他已完成卡共用的基类行为 |
| N2 | T0.36 的“本卡路径统一为 T0.36” | `release.py` 改后 `--suite package` 将不再匹配 T0.27 卡号。这对已完成卡无影响，因历史卡以受验提交为基准；实现时可让卡号取当前卡，便于后续复现 |
| N3 | T0.35/A1 描述（沿用 round-50 N3） | 描述中的“安装”依赖正文说明限定，下次修订时可统一文字 |
| N4 | TASKS.md:3、ARCHITECTURE.md:3（沿用 S12） | 状态行仍为旧进度 |

## 实际验证

运行了 `git rev-parse/status/log/diff`、`task.py status`、`task.py check`；逐行审阅 `dc49b61..78dee28` 的 TASKS、DECISIONS 修订；对照 round-50 已通读的 `t23.py`、`release.py`、`t10.resources`，并复查 `t14.py` 的 suite 写入位置。未修改任何文件，未运行写操作命令。

## 未验证项

- T0.27 package 重验、T0.36/A2 及全部 T0.28–T0.36 业务验收：NOT RUN。
- `t23.py` 的 `package`、`lifecycle` suite 尚未实现，本轮按卡内义务和现有代码推演。
- 真机或虚拟机的可用性由用户在 T0.36 提供。
- 文档 CLEAR 只确认规划充分，实现由后续验收证明。

DOC-REVIEW-VERDICT: CLEAR
