# Round 28：T0.9 第四会话授权独立文档复审

结论：ADR-029 与 PROGRESS 准确落实本次人类批准。预算仅为 T0.9 累计3→4，修复范围、历史保持及完整质量门明确，未发现阻塞文档问题。业务仍待修复后完整复验。

## 受审基线

- 项目及所有命令 cwd：`/Users/eason/jp/application`
- 固定 HEAD：`5b8b934addfc22f4097b65b2138e38cad65e6f9f`
- 实际 source：`ad2b5de36d1616ff00d2c4423366038dbbbc9ae976904c8713f28d88220d3e1e`
- 实际 protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-029 分段 SHA256：`8a8b072d7f01fb2418d7b3616d5f1697cdc50ebf6437d2bafc24ed272dbc2b5b`
- ADR-028 分段 SHA256：`46c40a9b9cfbc2aed67690189d5f787e4a860d8c1c73a1649c07a5eecd41ee27`
- 全部此前 ADR 分段摘要与父提交保持一致。
- 工作树干净；固定提交仅向 DECISIONS、PROGRESS 各追加8行，均保留父提交完整字节前缀。

受审文件 SHA256：

| 文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `cd013d28653b73cd84731367540b883d6dc7e7098f24dd3306874c1963cdc68b` |
| docs/PROGRESS.md | `f6d2c1a3b405e02c4a497e409a77792e03a885b6b831fd3b1241f66b58e8e5d1` |
| docs/TASKS.md | `c86035c76fc4bf3344875d236e5209c5a82761adf1efc42907c768cfa7678c3f` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |

## 授权、需求与最小修复

已读取 moc-init、项目 AGENTS、文档审查提示、round27、相关 SPEC/契约、T0.9 卡、ADR029及对应进度记录。本轮为既有 CLEAR 基础上的授权增量复审。

人类批准由 root 可信转述：“批准 T0.9 再追加一次”，随后“继续”；问题明确限定一次3→4、两项测试修复及完整复验。ADR029来源与记录相符。

两项修复均符合当前真实问题：

1. `tests/fixtures/n00.rs`、`n01.rs`分别加载同一 `observation_redaction.rs`，完整verify的Clippy明确报 `duplicate_mod`。由既有范围内 `host.rs` 的 `cfg(test)`入口声明一次，两fixture引用同一模块，保留helper行为与回归，是直接修复。
2. `protocol/app.schema.json`规定 `ImageAccessMap`为对象；N01新增夹具在1371行按 `/data/images/0/url`查找后unwrap，后续又按数组遍历。改为resource_id映射及values遍历符合既有DTO，保留先验schema、原响应不变、稳定字段、URL正负例及非法原值拒绝。

T0.9原13项需求、16原生行为、8独立观测、资源与安全断言均保持。host.rs已在原卡范围；产品DTO、鉴权、冻结验收及控制器保持。source/protected与记录一致，无冻结修订，无需rebaseline。

## 预算、状态与质量门

实际状态读取结果：

- T0.9：sessions3、失败10、generation缺省0，第四许可尚未消费。
- T0.13：paused=true、sessions1、失败0、generation3。
- workflow默认max_sessions=2；仅本卡唯一一次原 `drive.py --once --max-sessions 4`，计数4即消费。
- 原开发workspace-write、巡检read-only、4096MiB macOS RSS采样、jobs2、测试线程1、既有共享锁、全部时限及清理保持。
- 10次失败包含两次外层误判，WIP、失败收据及日志均继续保留；T0.10按严格顺序等待。

实施前仍须紧邻复核current/next/严格最早=T0.9、原计数、源码及冻结摘要、干净树、旧身份结束与两锁空闲。本审阅未获取锁或核验活跃进程。

修复后要求原CLI真实相关自验，沿ADR010处理已确认权限边界；新源重新完成原构建、all-targets Clippy、共享脱敏及相关新旧回归、准确A1，随后直接完整verify，再原done、受验Git完成提交、固定提交独立CLEAR及干净树检查。文档保留完整门，可行性推演的下一步为此次已批准修复会话。

## 已有证据与验证限制

只读核对本次decision-evidence清单，42份副本全部符合SHA256。

已有同源收据确认：

- 准确A1 finished、exit0、32JUnit通过、0失败、0skip；PROGRESS记录16原生全过、19必需case、8观测及28篡改拒绝。
- 完整verify前五门通过；第六门fmt/check通过后Clippy `duplicate_mod`失败，exit1，后续全量native test未到达。
- N01旧回归exit101，日志确认1371行panic；18/19结果与进度记录一致。
- N01原封存日志中符合43字符cap查询串形状的出现次数为200；本轮仅输出次数，未输出原值。保留原证据，继续落实既有脱敏门。

实际仅执行文件阅读、Git只读检查、原摘要API、状态字段读取、清单副本哈希及日志定位。Python仅使用 `/Users/eason/jp/application/.venv/bin/python -B`。未编辑、测试、启动CLI或获取锁。第四会话实现及新源准确A1、完整verify、done、完成提交与独立完成巡检均为NOT RUN，业务保持待验。

DOC-REVIEW-VERDICT: CLEAR
