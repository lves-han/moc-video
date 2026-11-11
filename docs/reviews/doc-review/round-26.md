# Round 26：执行器异常清理修订与双卡迁移独立复审

结论：本次批准范围内的增量修订证据齐备，原首次及清理枚举异常导致漏回收、空引用提前封存的问题已获得对应修复与故障回归。未发现阻止当前 T0.6 重新验收的文档或授权控制问题。本结论不代表 T0.6 业务质量门完成。

## 受审基线

- 项目：`/Users/eason/jp/application`
- 固定 HEAD：`4a98563cf872ac349122a1fed352cdbcaf937b51`
- source：`0e96d6015d635dbdb919efc7abae2d0cdebc429ca7e5192526d18b0ab1a41bcc`
- protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-027 分段 SHA256：`428a4bc585fe0107c643716f76e6857e19534886ff647556d02477fe7495833d`
- 实际只读命令均在上述项目目录执行，Python 使用 `/Users/eason/jp/application/.venv/bin/python -B`。未运行测试、网络、开发 CLI 或修改文件。
- `git status --short` 无输出。

所审文件 SHA256：

| 文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `f3bb2cca940174d60a666c06fb505bedf4a172fbddad270486d3fa630e8f6f72` |
| docs/PROGRESS.md | `3f3e26ad81349d0cf830b1e0952c7c6dce2bd329c950c6d3edf709a8c4262926` |
| scripts/platform_runtime.py | `be84378d6bac0ae8cd3bdf0302f7ed9b31217cda892f88ad393a091b96fbddae` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| scripts/tests/test_platform_runtime.py | `e361fcef8ec74c2ee6b5ce07b83c56be967524c5254d894f5e783737929a3ebe` |
| scripts/tests/test_workflow.py | `f0b059f0f436998fb4fa4d64d867bdffcfcf5cd34bb8d9edd2806d7598aea07d` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |

## 范围与授权

按 moc-init 技能及项目 `docs/PROMPT-DOC-REVIEW.md` 执行固定实际版本的增量复审。人类批准由外层可信转述，并与 ADR-027 记录一致：仅修复原 platform_runtime、workflow 及对应测试，沿现有入口迁移 T0.6 与暂停 T0.13，再重新验收。

当前提交实际修改四个批准控制文件及追加式 PROGRESS。未修改迁移实现、业务需求、验收范围、workflow 配置或默认预算。原 workspace-write 开发、read-only 巡检、默认两会话、4096MiB RSS 采样、jobs2、测试线程1、项目重任务锁和单条时限保持。严格顺序保持，T0.7 等待。

## 原 P1 闭合审查

1. **创建后首次枚举失败**：POSIX 在首次 poll 和 session 枚举前取得直接 Popen 子进程创建时间，登记 anchor 并调用 observe。原“已 launch 但 payload_refs 为空”的窗口不再依赖首次枚举成功才能登记。

2. **枚举与清理同时失败**：finally 将 session 清理失败保留为明确错误；仍尝试核对创建时间及 SID/PGID后清理初始组，随后独立清理已知身份子进程，并对直属 Popen kill/wait。枚举失败不再跳过回收与 wait。

3. **身份及 PID 复用**：fallback 组清理要求原创建时间、独立 session leader 和初始组同时匹配；已知进程清理复用原身份核对。session_family 对已复用 leader PID 拒绝授权清理，未增加以裸 PID 扩大清理范围的路径。

4. **不确定边界与重入**：PayloadCleanupError 在原 heavy 锁内先发布 cleanup_failed；新 executor 获锁后再次检查该屏障。最终状态保留未完成、不写 log_hash，避免将清理不确定性封装成正常 finished。

5. **恢复与日志封存**：run_alive 除已登记身份外，实时检查原 POSIX session，覆盖 leader 已退出、未观察子进程另建进程组的情况。枚举失败保持不能确认；recover 只有在边界确认结束后才能将失败以 exit125 归档并计算日志摘要。新增测试同时检查恢复拒绝、恢复成功及封存后日志稳定性。

未发现本轮范围内仍需修订的 BLOCKING 问题。此处结论基于实现及故障测试阅读，未以绿测替代因果链审查。

## 原迁移与历史保持

读取 `evidence/drive/runtime-cleanup-repair-20261029T022842Z` 的迁移日志及 before/after-nodes：

- 原入口日志明确迁移 T0.6、T0.13 两卡成功。
- 逐字段差异仅有 protected、generation、rebaselines、append_only、rebaseline_scope。
- T0.6 sessions1、失败2（含原实现红测）保留，generation0→1。
- T0.13 sessions1、失败0及 paused 保留，generation2→3。
- 原 base、segments、开始时间等其他字段保持，未重置历史或预算。
- rebaseline_scope 对应批准的四个控制文件；暂停卡历史 scope 保留。

原失败 A1 `20261029T015844789474-A1` 的收据仍保留旧摘要 `f70443…9ab89`，实际全日志摘要仍为 `2e4abd…9c73`。两者不匹配这一历史事实未被回写掩盖。PROGRESS 已追加纠正“未创建 payload”的旧推断，同时保留原生业务当时未执行、历史退出时间与用量未知的准确边界。该旧失败不能作为当前成功证据。

## 已有验证证据核对

本审阅者只读已有日志和收据，未重新执行测试。

- `20261029T023750424218-runtime-cleanup-tests`：exit0/raw_exit0，128项、121通过、7平台跳过；source/protected/generation 与本轮一致。
- 上述完整日志 SHA256：`52189d6e00356cbda57363f38589163678f37b47b9002f6acbbe7233302332fa`，与原收据一致。
- `20261029T024002173220-runtime-document-check`：exit0/raw_exit0，128项、121通过、7平台跳过；source/protected/generation 与本轮一致。
- 上述完整日志 SHA256：`9f0670be22fdff35cae21f19ee7d7230739d0068eb74d631c74c81a674c50d4e`，与原收据一致。
- `evidence/script-validation.json` 记载 macOS arm64、Python3.12.14，四入口实际退出码与预期一致，业务验证明确标记 NOT RUN。
- 只读扫描201份运行收据：未完成收据为空；按 PID＋创建时间核对执行器、payload_refs及存在的 launcher 记录，未发现仍存活的匹配身份。
- 原锁路径及配置保持；本次未通过获取锁来独立重验锁空闲状态。

## 未验证及后续门

本轮运行证据来自当前 macOS arm64。七项跳过对应 Linux/systemd/cgroup 或 Windows 专属行为；Windows 故障清理路径未实机验证，不能由本轮结果推定跨平台通过。

当前新基线下的 T0.6 准确 A1、完整 verify、原 done、受验 Git 完成提交及原 drive 固定独立复巡尚待执行。旧成功 A1、verify、done 和原 BLOCKING 巡检均保留历史，不替代这些新门。T0.7 须继续等待。

DOC-REVIEW-VERDICT: CLEAR
