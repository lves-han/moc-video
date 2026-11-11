# Round 29：T0.10 第三会话授权独立文档复审

结论：ADR-030及对应RESOLVED准确落实真实人类授权。修复直接针对隐式IV素材与校验缺口，单卡预算、修复范围、历史保留和完整复验门明确，未发现阻塞文档问题。真实隐式IV业务验证仍为NOT RUN。

## 受审基线

- 项目及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`2389e053ec90692fcfc88b428a97572354a6fc19`
- 实际source：`84a54c2242a250278993421cdcf8c4c4523d0340d516391304ded9aa33e7f2f1`
- 实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-030分段SHA256：`cf95dc1bc8f9b096d1c4549bd0c8c5b6cbc58978973bd005d1e7b93d5b8d5549`
- 工作树干净。固定提交仅向DECISIONS、PROGRESS各追加8行；两文件均保持父提交完整字节前缀。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `864d2fc61ee3564a117b570a062ecda19fe19e03a79ddd69fb4b7abbf5d9429a` |
| docs/PROGRESS.md | `b0b9e539aa9b3b35d37d541f87928e68ef97253890fc6bfd64d469c86bed1a0b` |
| docs/TASKS.md | `b6d2c43b5c98e9e62661fc8fa6a5f975f89d4c10db2e0d239ce69e51e865edf1` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |

## 授权、需求覆盖与最小修复

已读取moc-init、项目AGENTS、PROMPT-DOC-REVIEW、round28、SPEC9.3、C8、T0.10完整卡及覆盖关系、ADR030与相关PROGRESS。本轮为既有审查基础上的授权增量复审。

root可信转述的真实人类回复为“批准 T0.10 追加一次”，对应问题明确限定2→3、逐片序列号加密素材、前置断言、独立校验及完整复验。ADR030与RESOLVED准确记录该授权，无新增产品范围。

已确认缺口的因果链清楚：标称aes-implicit的既有素材包含显式IV=0，历史成功支持该素材播放；真实隐式IV仍待验证。完成勾选已撤回，BLOCKED及随后真实RESOLVED均保留，旧成功结论已有明确纠正。

ADR030最小方案足以覆盖SPEC9.3、C8和T10的IV与序列义务：

- 每片以非零媒体序列号对应的大端128位IV独立AES128CBC加密，清单无IV属性，明确排除仅删属性并沿用固定IV密文。
- 增加素材前提、独立oracle及篡改反证，拒绝显式IV误标与序列加密错误。
- 保持原本地HLS和原网关两路各完整24视频帧逐帧hash等于原源、非零音频、显式/真实隐式/fMP4三模式。
- 保持全部派生请求同cap、原来源上下文、序列、安全与负载门，以及原8行为、17观测、3需求。
- 修订仅在既有tests范围，复用已有ffmpeg/openssl；生产DTO、鉴权、冻结卡与控制器保持。

实际source/protected与文档一致。此次为既有义务下的素材准确性修复及单卡额度授权，无冻结定义修改，沿现行流程无需rebaseline。

## 预算、状态与路线可实施性

实际只读状态核对：

- T0.10：sessions2、失败2、generation缺省0；第三许可尚未消费。
- T0.13：paused=true、sessions1、失败0、generation3。
- workflow默认max_sessions=2保持。
- 仅一次原`drive --once --max-sessions 3`；计数3即消费本次许可。
- 原权限、4096MiB macOS RSS采样、jobs2、测试线程1、项目原两锁、全部时限及历史保持。

当前最早待完成卡为T0.10；T11按已批准严格顺序等待。启动前仍须紧邻核对current/next/严格最早、原计数、clean/source/protected、旧进程身份退出与两锁空闲。本审阅未获取锁或核验活跃进程。

开发真实相关自验证及ADR010受限代验路径明确；修复后新源须重新完成原构建、Clippy、相关回归、准确A1，直接完整verify，再原done、受验完成提交、新固定提交独立CLEAR及干净树。文档保持完整完成门，无通过历史证据直接推进的路径。

## 证据与验证限制

已只读核对`evidence/drive/t10-current-validation-dir.txt`及其指向目录内`implicit-iv-gap/manifest.json`。清单所列6份副本全部符合SHA256，覆盖原done、A1 JSON/XML、独立巡检报告/状态及原native日志。原成功、失败、受验提交与CLEAR继续保留历史。

本轮仅执行文件阅读、Git只读检查、原摘要函数、状态字段读取及副本哈希核对；Python仅用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件/状态/Git、未测试、未启动CLI、未获取锁，未输出凭据原值。

第三会话修复实现及新源相关自验、准确A1、完整verify、done、完成提交和新独立完成巡检均为NOT RUN，后续按上述门执行。

DOC-REVIEW-VERDICT: CLEAR
