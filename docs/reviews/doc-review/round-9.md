# 独立文档复审 round-9

本轮仅审查T0.1追加一次开发会话的授权、原脚本参数语义及后续验收闭环。审阅者全程只读，未运行drive、run、verify或validate，未写入项目或启动子代理。

## 受审基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`4f9e23e5708bd9206eba5e935d1dc869004b2822`。
- 当前受验源码摘要：`8ce9ac05f0da3a12f3bd1f418e479c3459edb3f3482f677301eaeb37470c94f3`。
- protected：`bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a`。
- current及next均为T0.1；sessions=2，workflow默认max_sessions=2，generation=0。
- 原2条失败、两次开发会话及全部收据保留，hard_sessions为空。
- 当前定义变更为追加ADR-011、真实RESOLVED及原handoff；原受保护文件保持。

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
| ADR-011 | `48be2fa1e537f273c097ea8a1740ed0d93e8ea0dc3fa22abb8ea87dee831297b` |

主要文件摘要：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `bc0cf0f076e33bf1684f66ece496c6792bc21333c2f9af6e0fbe8ece6d571ac5` |
| docs/TASKS.md | `4624055b45ad42e092feed09aa793a954066ee835f1a8c3c5247b13fbfdafa84` |
| workflow.json | `d28b4ef54bb71c7cc3720c90e3233bb9922ca2560a06a2f01750a73ed3d1cb31` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| docs/PROMPT-CARD.md | `62d776ad2ae53a6bd8cc8d774215f4a0b789c9f65a037b478140249ca1d6beed` |
| docs/DECISIONS.md | `30322124ab37a3ccc9cad1e20122bee4ffc2c45279ea2efb9ceeb6c1d6c10192` |
| docs/PROGRESS.md | `30fcc776a92678df1dca6799800c743db5064076933d30cdbce2bc16d0a7f2b8` |
| docs/runtime.lock.json | `9c165538eaf9364defc55a3a59b3125a2e4b4cae8aa6777d1ce7987c96d814e7` |
| tests/acceptance/t00.py | `b9bc3f3ca6005b775fa4169fecd1774b57e518d50d17c974e6d023eb013f7160` |
| evidence/task-state/T0.1/handoff.md | `911ac823402212c946f997c3ebae4ce2bd083f0f4a34eccfd4b43e11a38e6c2a` |
| docs/reviews/doc-review/round-8.md | `7a251c22eb094c634ec330246c88059c2fc62effcf3714df7577bd411f674a06` |

## 授权结论

外层提供的最新人类原话：

> 同意，仅将 T0.1 的开发会话上限从 2 次增至 3 次，让原脚本修复版本漂移问题，你负责复验，现有权限和资源限制保持不变。

ADR-011、同卡RESOLVED与handoff准确保留该决定，范围均限于T0.1的一次版本漂移修复。ADR-010的原脚本开发、外层复验分工继续。

这是具备直接人类来源的单卡预算例外。原默认禁止擅自扩额的规则仍适用于其余情况；当前记录未将授权扩大到其他卡、额外时长、权限、资源或质量门。

## 原脚本参数可行性

已读取原`drive`实现：

- 第1182至1195行将`--max-sessions`解析为本次调用的局部maximum，未写回workflow配置。
- 第1301至1305行沿既有累计sessions检查并递增；当前2次，在maximum=3时只产生第3次会话。
- 第1373至1384行在`--once`模式下，无论该次开发会话形成DONE或WIP，均结束本次调用。

因此，在外层紧邻启动时确认current/next=T0.1、sessions=2、无存活写入者，并使用唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 3
```

该调用只启动当前T0.1的一个开发会话。T0.1结束后，后续卡继续使用普通`--once`，从原workflow读取默认上限2。独立`--review-only`保持原语义，不增加开发会话。

原protected及generation均保持，原`check_frozen`通过；此次通过运行参数落实真实授权，无需改受保护配置或调用rebaseline。旧失败与计数保持，修复后仍须以新源码重新验收。

## 失败因果链与修复范围

已核验三份原收据及支撑日志：

1. `20261027T020048523187-t00-rust-toolchain`：exit=0；安装日志明确记录从Rustup 1.29.1检查并下载1.29.2自更新。
2. `20261027T020237077067-t00-prepare`：exit=0；准备成功证据保留。
3. `20261027T020418934918-A1`：exit=1；源摘要前后一致，protected及generation与当前相同。JUnit记录24通过、7失败、4跳过。

原日志与`findings.md`一致：严格版本门报告`rustup: version drift from baseline`，随后Core最小构建因版本门失败记录NOT RUN。原锁中的observed_tools.rustup仍为1.29.1，安装argv缺少禁止self-update的选项。

追加会话用于修正准备命令的自更新副作用、同步真实版本记录并补相关回归，直接对应已复现问题。当前授权保留严格版本、原包、源码、资源及JUnit门，未要求放宽断言或伪改旧报告。

外层保留的7包身份通过、准备通过及验证器行为结果，分别证明对应已执行部分；准确A1仍为FAIL，完整verify及完成提交/独立巡检尚待修复后执行。

## 约束继承与收尾

原需求、平台、架构、任务、工作流和运行控制器摘要与round-8相同，其覆盖、环境、性能、路径及真实集群约束继续成立：

- 29桌面卡、6移动卡及原阶段门保持。
- 必需平台仍为本机macOS arm64。
- 原性能指标及其强度、样本和阶段保持。
- 开发workspace-write、巡检read-only、gpt-6.1-sol保持。
- 单次10800秒、原命令超时、4096MiB RSS采样、jobs=2、测试线程1及全局共享锁保持。
- 原准确A1、全部JUnit cases、完整verify、done、受验完成提交及固定提交独立CLEAR保持。

第3会话结束后，外层在源码稳定、旧进程退出后，按ADR-010通过原执行器执行新准备及准确A1。准确A1全部通过后再运行完整verify并按原流程收尾。继续需要开发时，沿原规则报告新决定，保留此次第三会话及失败历史。

## 实际验证与未验证项

本轮实际完成：

- 原`task.py check`、`scope`退出0。
- 原只读`check_frozen`通过，next=T0.1。
- sessions=2、默认max_sessions=2、generation=0及2条失败均核对。
- 原budget_problems返回空问题及空提示。
- 原安装/prepare/A1收据及日志摘要核验；A1报告副本摘要匹配。
- 三次外层执行器及记录的payload身份均已退出。
- heartbeat任务`automation`仍为PAUSED。
- 审前、审后protected及原会话状态保持。

本轮未执行修复、准备、A1或完整verify。当前准确A1为FAIL，Core最小构建仍NOT RUN；修复效果及最终完成资格待原脚本实现和外层真实复验。

未发现本次单卡预算授权与原脚本、冻结、计数、资源及质量门之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
