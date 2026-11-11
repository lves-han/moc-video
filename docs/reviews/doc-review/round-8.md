# 独立文档复审 round-8

本轮为独立只读复审，重点审查新增开发/验收分工与原工作流的兼容性。审阅者未修改项目、启动开发或巡检，也未运行 drive、run、verify、validate；检查使用原 task 只读入口、原控制器的只读函数、现有日志和收据。

## 受审基线

- 项目根：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`5e20741`，前序实现WIP为`10fbe00`，启动基线为`d2122afb0f0861f45e62cda3db5a3eed27df12e7`。
- 当前未提交修改仅为追加`docs/DECISIONS.md`及`docs/PROGRESS.md`，合计15行。
- 原handoff已更新，位于`evidence/task-state/T0.1/handoff.md`。
- 审前、审后protected均为`bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a`。
- current=`T0.1`，sessions=1，max_sessions=2，generation=0；保留1条原失败，hard_sessions为空。
- 原`check_frozen('T0.1')`通过，`task.py scope`报告`all changes inside scope`。
- 当前blocked为空，原next_card仍选择T0.1。

ADR分段摘要：

| ADR | SHA-256 |
|---|---|
| ADR-001 | `31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407` |
| ADR-002 | `d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368` |
| ADR-003 | `26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c` |
| ADR-004 | `a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae` |
| ADR-005 | `df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d` |
| ADR-006 | `335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1` |
| ADR-007 | `028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227` |
| ADR-008 | `9c1b6bc1afa7ae46d9c714f2da6ff25775bb4f84fa2a894666823fabbc9f00cc` |
| ADR-009 | `2af85af3e9a0d33cd2fe5088a27bc3075ed73986e2e6da5b904129753f6e62c6` |
| ADR-010 | `2a8794076a5170988b290a41e762b733d1fca797f403b3ec2a497e32cbf4f45b` |

当前受审文件摘要：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `bc0cf0f076e33bf1684f66ece496c6792bc21333c2f9af6e0fbe8ece6d571ac5` |
| docs/TASKS.md | `4624055b45ad42e092feed09aa793a954066ee835f1a8c3c5247b13fbfdafa84` |
| docs/DECISIONS.md | `6b061f0ba1b5e2d4d7fbee61b2fa0a6aa04e3ff955aed32f6fe5db47bc6eac55` |
| docs/PROGRESS.md | `fba1ad1c0191231d2bf345b5c0560e9496bc832aff999634eca322d67cfe5a43` |
| workflow.json | `d28b4ef54bb71c7cc3720c90e3233bb9922ca2560a06a2f01750a73ed3d1cb31` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| scripts/tests/test_workflow.py | `596487772cb4f71cdd015c34fc18ed86f6a8bfec76ff57dc4178fee9c9e1378d` |
| docs/PROMPT-START.md | `f72455f542af1f92f63c641a342cbcec286b49479356dd56c444612eb39e1d0a` |
| docs/PROMPT-CARD.md | `62d776ad2ae53a6bd8cc8d774215f4a0b789c9f65a037b478140249ca1d6beed` |
| docs/PROMPT-SUPERVISE.md | `2089bb44078270100a9299d4de55cdee62bb43c3a88a3253ef382abed020f88a` |
| docs/PROMPT-DOC-REVIEW.md | `399371107d92e4172312182ba7e888fbe41f377a35c57d6109ccb852ef7382c0` |
| protocol/app.schema.json | `6ab4da36b7b7e32be41a1cc1eb5581a72da2588022d0bc964e0b45cd2a098204` |
| protocol/bridge.openapi.json | `55c3e4ac4c5a7fd0c18e9dcca5c602236450437a48361f59641c48595564110d` |
| protocol/examples/contract-cases.json | `6df0d8d48c541bb9a6194373ff30d5ad7a491c8bd3b576c3019001bde2711ff7` |
| docs/reviews/doc-review/round-7.md | `52ddb5bbc328a1b62d32d3fb6b69601e3255019fac145d5a09f1b8efab676369` |
| docs/runtime.lock.json | `8bd4674aeede738b0036802afa44ec54fd6342173092fd0a7136e84fe81bd999` |
| evidence/task-state/T0.1/handoff.md | `f21be538887944a083fe78fd8e6e1f37540405340201c4acd111fe322396b98c` |

## 授权与分工结论

外层提供的当前人类原话为：

> 你作为agent负责巡检验收，最小化授权  script里的脚本负责开发实现

该原话与ADR-010、T0.1最新RESOLVED记录一致，handoff保留来源并给出可执行分工：

- 原`drive.py --once`及其开发CLI负责本卡代码、验收脚本和必要清单的实现。
- 开发CLI继续使用`gpt-6.1-sol`与workspace-write；独立巡检继续read-only。
- 外层在开发及重命令退出、源码稳定后，通过原`run.py`执行构建、依赖获取及准确A1，再直接调用原`verify.py all`。
- 外层核对原收据后按既有Git授权收尾，使用原`task.py done`及完成提交，随后原`drive.py --review-only`取得独立结论。
- 实现问题交回原开发工作流；外层保持巡检验收角色。

分工处理的是已实测的执行前提差异，未增加CLI权限、变更共享锁位置、调整预算或建立第二控制系统。当前用户决定具有比旧通用会话提示更高的优先级，原handoff能将该决定传入剩余开发会话。未发现新增角色或接口与原质量门之间的冲突。

## 冻结与剩余会话可行性

原T0.1受保护定义保持；ADR和PROGRESS按追加规则记录真实决定，原`check_frozen`已通过。当前RESOLVED解除的是执行分工待决定事项，未改变任务正文、验收、配置、脚本或资源限制，因此当前分工可沿原基线与generation=0继续，rebaseline无必要。

原控制器流程支持此次接续：

1. 原next_card已恢复选择T0.1。
2. 原drive检查`sessions < max_sessions`；当前1<2，下一次`--once`递增为第2会话。
3. 第2会话提示会带入原handoff及journal；本次剩余开发预算仍为一个10800秒会话。
4. CLI以WIP退出时，原drive返回once_wip，保留本卡状态；该结果只触发外层读取交接。
5. 外层原run.py从同一current、任务定义、protected和generation登记收据。原完成检查按准确argv、source/after_source、spec、generation、protected、JUnit及日志核验，未要求调用者必须是仍存活的开发CLI。
6. 外层完成A1、完整verify和原done后提交受验源码；原`--review-only`检查完成提交并启动只读巡检，该模式不增加开发会话。

原失败后已经有更新的note，当前hard_sessions为空。失败收据、首次会话及WIP历史保留，未发现额度或失败记录被重置。

## 需求、环境和运行约束复核

本轮通过摘要复核，确认round-7已审的AGENTS、SPEC、ARCHITECTURE、TASKS、workflow、运行控制器、测试及协议定义保持。进一步只读核对：

- 113条覆盖记录及其卡号/验收引用完整。
- 35张卡、36项验收，冻结cases仍等于对应卡需求ID列表，JUnit要求保持。
- 16份原始输入摘要全部匹配。
- T1.1及其五张依赖移动卡继续由阶段门冻结；当前分工没有移动授权含义。
- 必需目标继续为本机macOS arm64，其他桌面平台按ADR-005/006保留NOT RUN及后续复现要求。
- 原性能目标、单位、样本规模、适用阶段及“目标/本机校准”的强度保持。
- 项目根、venv、原脚本、driver锁及evidence绑定保持；原全局heavy锁仍为`/Users/eason/Library/Caches/agent-project-workflow/heavy.lock`。
- 4096MiB、jobs=2、测试线程1、原命令超时、每卡2会话及RSS采样后端保持。
- 真实集群Docker成员、逐容器硬内存、容器限额合计加宿主RSS、实际用量和自产清理的契约保持。
- 原budget_problems返回空问题及空提示。既往长卡实测时长与单会话装箱提醒继续适用。

## 现有执行证据

原CLI内探测收据：

`evidence/runs/20261027T010843115773-t00-resource-probe/receipt.json`

其结果为finished、exit=2；原payload未启动，记录全局锁写入EPERM。原失败保留在T0.1第1会话。

外层探测收据：

`evidence/runs/20261027T011517912651-t00-resource-probe-outer/receipt.json`

其结果为finished、exit=0，准确执行打印payload；protected及generation与当前基线一致。审阅者通过原只读`verify_evidence`核对其支撑日志。

该探测的ID为`t00-resource-probe-outer`、kind为command，既非声明的A1，也未提供A1的JUnit cases。它证明外层原执行器可执行该最小payload；业务验收和完整verify保持NOT RUN。

审阅者按原`platform_runtime.identity`核对以下已记录身份：driver8306、开发包装器23551、真实CLI23555、失败执行器25633、外层执行器31853及其payload31858均已退出。

当前聊天heartbeat任务`automation`的持久化记录为PAUSED，目标聊天与项目绑定一致。当前尚未恢复第2开发会话。

## 实际检查

本轮实际执行及结果：

- 原`task.py status`：T0.1、sessions=1/2、generation=0、1条失败、blocked为空。
- 原`task.py check`：退出0，35 cards。
- 原`task.py scope`：退出0，all changes inside scope。
- 原只读`check_frozen('T0.1')`：通过。
- 原next_card：T0.1。
- 原budget_problems：无问题。
- 原blocked/frozen_cards：当前卡已解除执行分工阻塞，移动阶段门保持。
- 受审文件、全部ADR分段及原始输入摘要复核。
- 阅读run_command、completion_evidence、done、drive及PROMPT-CARD，核对外层执行、WIP接续和review-only的真实语义。
- 核对新ADR、RESOLVED、handoff与外层转述的人类原话。
- 原成功probe支撑证据及旧进程身份核验。
- 审前及审后protected、会话数和失败历史一致；审阅者未造成文件变化。

## 执行提醒与未验证项

- 保存并提交本报告、ADR-010和真实RESOLVED记录后，复用原heartbeat任务及原`drive.py --once`恢复T0.1第2会话。
- 开发交接应给出源码稳定状态、准确构建/验收argv和实际NOT RUN清单；验收程序应将运行证据写入evidence，保持受验源码摘要稳定。
- 准确A1、完整verify、完成提交及固定提交独立CLEAR仍须全部取得；历史启动验证或probe成功保留各自证明范围。
- 第2会话为当前卡最后一个已批准开发会话。若其结束后出现需要继续开发的实现问题，按原预算停止推进并报告所需决定。
- 本轮未执行开发、业务验收、完整verify或真实集群测试。当前T0.1七包核验、Core最小构建、验收脚本与完整许可/版本锁仍待原开发会话及外层准确验收。

未发现新增分工与原冻结、资源、预算、收据、完成提交和独立巡检质量门之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
