# Round 27：T0.9 单次追加预算与测试修复授权独立复审

结论：ADR-028 与 PROGRESS 的追加记录准确落实人类针对 T0.9 第三次开发会话的批准，最小修复范围、历史保持和完整质量门清晰。未发现阻止按此授权启动一次原开发会话的文档问题。当前业务验收仍待修复后重新执行。

## 受审基线

- 唯一项目根目录：`/Users/eason/jp/application`
- 固定 HEAD：`c956f9cf7f749cf0b1f7eed5a9f34fccfccc2854`
- 实际 source：`305a1333ff06844af64028e2dae881038af457e27faebf6822c758b55dd21615`
- 实际 protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-028 分段 SHA256：`46c40a9b9cfbc2aed67690189d5f787e4a860d8c1c73a1649c07a5eecd41ee27`
- ADR-027 分段 SHA256 保持：`428a4bc585fe0107c643716f76e6857e19534886ff647556d02477fe7495833d`
- 使用原 `workflow.snapshot()`、`workflow.protected_hash()`、`workflow.adr_digests()` 只读计算；此前 ADR 分段摘要全部保持。
- `git status --short` 无输出。固定 HEAD 仅追加 DECISIONS 8行、PROGRESS 9行；两文件均保留父提交的完整字节前缀。

实际受审文件 SHA256：

| 文件 | SHA256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| docs/TASKS.md | `c86035c76fc4bf3344875d236e5209c5a82761adf1efc42907c768cfa7678c3f` |
| docs/DECISIONS.md | `9252688cc9327793d3cdc8025c50c74d14446479454ed647c0316335538180f2` |
| docs/PROGRESS.md | `5cc9d03276484434c7597262b579c7e672cea921138535a02db5eb3447e069d9` |
| docs/reviews/doc-review/round-26.md | `a802778d8f117c47400ebf82bc58b6e27e520143726d9b45334dcbd44378b4be` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| scripts/platform_runtime.py | `be84378d6bac0ae8cd3bdf0302f7ed9b31217cda892f88ad393a091b96fbddae` |
| scripts/drive.py | `96b6129dcf9dedf08880eba2270f869c3c9f9e74796d0874f7cba0795db93b8b` |

## 授权与需求覆盖

人类批准由外层可信转述，内容与 ADR-028 来源及 PROGRESS 同卡 RESOLVED 一致：仅批准 T0.9 追加一次开发会话，累计2→3，处理 gzip/Range 测试夹具、图片续期兼容断言和测试日志脱敏，再重新验收。

本轮增量保持 T0.9 的13项需求、16项原生行为、8份独立观测及原 A1 定义。受影响要求核对如下：

1. **R27、REQ-T09：gzip 与 Range。** 有效 gzip 正例须保持原解压字节；损坏 CRC、截断响应和失败诊断继续形成反证。Range 仅合并相同返回分支，保留200/206/416、If-Range、HEAD，且继续通过 Clippy。
2. **R37、R31：图片身份与租约。** SPEC §5.4 明确图片访问或被新 DTO 引用即续期30分钟；ARCHITECTURE 保留同 epoch 去重、滑动续租和4096 LRU。重复详情测试改为完整稳定内容、图片身份/path/cap相等，加上有效续期断言，符合既有契约。原200集、分组、顺序和 schema 断言继续。
3. **R12、R44：凭据隔离和日志。** 查询串 cap 与失败 Debug 均需脱敏；真实生产响应先验 schema 检查、合法非秘密观测占位、原始响应保持的要求明确，修复无需改变生产鉴权。

以上修复均处于 T0.9 已批准 `tests/` 范围。冻结需求、卡正文、验收定义及控制器保持，采用现有单次启动参数落实预算，当前授权无需 rebaseline。

## 控制、预算与历史保持

实际原状态为：

- T0.9：sessions2、失败4、generation 缺省值0，protected 与受审摘要一致；第三次许可尚未消费。
- T0.13：paused=true、sessions1、失败0、generation3。
- workflow 默认 `max_sessions=2`；本次仅一次 `scripts/drive.py --once --max-sessions 3`，计数达到3即消费许可。
- 原 gpt-6.1-sol 开发 workspace-write、巡检 read-only 保持。
- 原4096MiB macOS进程树 RSS 采样、jobs2、测试线程1、项目重任务锁保持。
- 原 session10800秒、idle2700秒、command7200秒、review1800秒及各项验收/verify 超时保持。
- WIP、失败、收据、日志及当前基线保持；无计数重置、增加会话时长、权限扩展或控制器变更。

ADR-015 严格顺序及 ADR-010 分工继续适用。实施前仍须紧邻核对 current/严格最早/next=T0.9、原计数、旧身份结束及两锁空闲。T0.10 等待 T0.9 完整闭环，T0.13 原暂停保持。

可行性推演的下一步为这一次已批准的 T0.9 修复会话；文档未要求新增外部凭据或环境条件。源码稳定后，经原 run 重新 locked 构建、all-targets Clippy、相关新旧回归及准确 A1；A1 成功后直接完整 verify，再原 done、受验 Git 完成提交、固定提交独立 CLEAR 与干净树检查。完整门均保留。

## 已有证据核对

只读核对 locator 指向的 `decision-evidence/manifest.json`：

- 26条副本记录全部符合清单 SHA256。
- 原位置中25条仍符合清单；唯一差异是动态 `evidence/task-state/T0.9/handoff.md`。
- handoff 副本保存第二会话结束时交接，当前 handoff 更新为06:08的预算阻塞与诊断交接。差异与 PROGRESS 时间及内容一致，原副本完整保留。后续引用该快照时应明确使用清单 `copy` 字段；当前 handoff 按最新交接读取。此差异属于动态交接更新，未改写受验源码或失败收据。

已有运行收据均显示 finished：

- locked 构建 exit0。
- Clippy exit101。
- 准确 A1 exit1；原生结果为15/16，外层 JUnit 汇总为2通过、14失败，两种统计层级应分别记录。
- gzip 诊断 exit0，确认64字节夹具 CRC 错误。
- 独立 oracle 诊断 exit0，8向量、28篡改拒绝；256MiB流 RSS 增长2097152字节，717样本。
- N00/N01/T06/T08 回归 exit0，T07 回归 exit101。

旧日志中符合43字符 cap 查询串形式的出现次数：

| 原日志 | 次数 |
|---|---:|
| `20261029T055905054791-t09-regression-n00/output.log` | 10 |
| `20261029T055907721844-t09-regression-n01/output.log` | 200 |
| `20261029T055927344360-t09-regression-t06/output.log` | 6 |
| `20261029T055931305988-t09-regression-t07/output.log` | 15 |

上述位置均在 `evidence/runs/` 下。凭据原值未输出。脱敏修复应覆盖 T06 复用的旧观测路径，并验证失败输出；原封存日志及 SHA 按授权保留。ADR-028 的“现有测试观测”覆盖该公共缺口，范围充分。

## 实际验证与剩余工作

本审阅者全部命令显式使用项目 cwd，Python 仅使用 `/Users/eason/jp/application/.venv/bin/python -B`。实际执行文件阅读、Git只读检查、原摘要API、状态字段读取、清单文件哈希和凭据出现次数检查。未编辑文件、获取锁、启动开发 CLI、运行测试或完整控制 validate。

旧 actors 结束及锁空闲沿用外层可信核对，本审阅者未独立获取锁验证；正式启动前继续按 ADR-028 重核。

第三会话修复、修复后准确 A1、完整 verify、done、完成提交与固定独立 CLEAR 均为后续工作，业务状态保持待验。当前已确认的失败作为修复输入完整保留。

DOC-REVIEW-VERDICT: CLEAR
