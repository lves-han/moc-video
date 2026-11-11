# 独立实际实现与文档复审 round-18

本轮独立只读审查已批准的原rebaseline批次扩展、实际三卡迁移及分期文档。审阅者未修改文件、执行测试或重命令、变更状态。

## 固定实际基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`7b5e577e91603a0cd333fa476323c6c5ce2b1fb7`。
- source：`f16b01eb80b4681de117c1c4dd58f5f1dc70c45e045374f78abf0305c163a599`。
- 新protected：`a13052cd3dfcc96cb36c056b2f41fcf31b88dcb3b4ce0b5347ffb4e804781db1`。
- 工作树干净，current/next=T0.2。
- ADR-020摘要：`4deb9a3d81c7033670e543644299bd456e0ec2eabfeeb25d96cb3e05b87fc9b3`。
- ADR-021摘要：`162b25bbf3de89b4a4c2fb1a6ed5ba2791fad963300807d7781f74e5e13025a9`。

关键实际文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| scripts/workflow.py | `b5f02bcddc18d257684802d494520408b9a5799dae307721242bea9aa806b7ec` |
| scripts/tests/test_workflow.py | `9045a716180651818404b191ff59eca2118ed3c05bae1581aca58a7b8c28c854` |
| docs/TASKS.md | `96a43c3dfefa602d22822ec4375a0962a45886a6843790b85e1837b7a0265269` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| docs/DECISIONS.md | `0be6695d5a6e8e536b6ccae7bda6d5d1a08a6045b86a105da5332619ce1a6acc` |
| docs/PROGRESS.md | `d69cbb1c78ee6c7135ef4104185da8a1d912ada46e2da44d1112b3a7f7f1729c` |

两份控制器实际文件与已审候选逐字节一致；TASKS/ARCH与round-17固定候选一致。

## 授权与实现结论

用户针对仅扩展原rebaseline、迁移三卡并保留历史、计数、卡序和暂停状态的问题，明确批准：

> 批准最小迁移并继续

ADR-021及实际批准摘要、暂停卡集合与该来源一致。

静态审查确认：

- 默认单卡入口保持。
- 批次入口要求准确覆盖全部已开始未完成卡，当前卡活跃、其余卡paused，批准集合和摘要精确匹配。
- 全程持有原driver及项目heavy锁。
- 驱动、开发进程、执行器、payload及未完成收据均在状态保存前核验；finished收据仍有活跃payload时拒绝迁移。
- 所有节点的前缀、scope、批准事件及原基线一致性验证完成后，才执行逐文件保存。
- 同一批准事件可恢复部分迁移，已迁移节点不重复增加generation。
- paused卡scope排除暂停后其他卡的已提交变化，继续检查其活动区段及未提交变化。
- 原current、paused、计数、失败和区段未被代码重置。

未发现具体实现阻断项。

## 实际迁移结果

迁移前后原状态及保存记录已读取核对：

| 卡 | generation变化 | sessions | 迁移时失败数 | paused |
|---|---|---:|---:|---|
| T0.2 | 1→2 | 6 | 18 | false |
| T0.4 | 0→1 | 1 | 0 | true |
| T0.13 | 0→1 | 1 | 0 | true |

三卡均更新为新protected，记录同一批准事件及完整迁移集合。原base、resume_base、segments、last_note、sessions、失败列表和paused在迁移中保持；current仍为T0.2。

T0.13顺序BLOCKED保留。T0.4已获host.rs范围落实并真实RESOLVED，仍paused等待前卡。原T0.1/T0.3历史完成证据继续由原completed核验成立。

迁移之后，功能隔离候选首次导入缺少相对server.py，原run追加第19条真实失败；补齐既有夹具后focused回归通过。该新增失败及相应note正常追加，原18条失败前缀完整保留，未发生计数回退或状态重置。

## 实际验证证据

已通过原只读verify_evidence核对支撑日志：

- `20261028T025600726273-migration-candidate-tests`：exit0，104测试、1平台跳过。
- `20261028T025951500653-migration-candidate-focused`：exit0，9项迁移行为回归通过。
- `20261028T030251568378-migration-applied-tests`：exit0，实际全部scripts/tests共125项，118通过、7平台跳过。

覆盖包含默认拒绝其他未完成卡、错误集合/摘要、前缀改写、非paused、活跃身份/收据/锁、scope越界、原基线不一致、部分保存恢复、锁持续持有、历史保持及finished收据活跃payload拒绝。

审阅者未重新运行这些测试；结论来自原收据、日志摘要及实际固定代码审查。

## 分期文档与剩余工作

实际分期文档保留round-17结论：

- T0.2开发功能验证复用open/controls/stop和真实状态读回，沿原操作超时，保留构建、相关回归、安全、布局及即时清理。
- 两格式各1800秒及20生命周期转由T0.28承接。
- T0.25原网关耐久由T0.26承接。
- 产品阶段原强度、原始证据链、独立审查和最终完成门保持。
- 原资源、权限、严格卡序、项目专属锁及其他卡默认预算保持。

本轮确认实际迁移及固定文档充分；functional业务代码尚未应用，准确功能A1、完整verify、完成提交及独立巡检仍待后续实现和真实验证。三卡新generation下的完成证据须重新产生。

DOC-REVIEW-VERDICT: CLEAR
