# 独立文档复审 round-10

本轮审查ADR-012的单卡预算授权、当前T0.3在途状态、原选卡/恢复语义，以及T0.2修复后的验收闭环。审阅者全程只读，未修改文件、运行drive/run/verify或重命令，未启动子代理。

## 受审基线

- 项目根：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`3e7ab44eba675d5fadd713b285c1689f1c668167`。
- 当前源码摘要：`bf32eb1ab5562ad5db4e4ec990fb2eedc3621cfcb65bba0b905ce23b63a1988f`。
- protected：`bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a`。
- 工作树干净；current及next均为T0.3。
- T0.3：第1会话WIP已提交`66bd498`，sessions=1，原默认上限2。
- T0.2：paused=true、sessions=2、4条失败保留，generation按原默认值为0。
- workflow默认max_sessions仍为2；原运行控制器、权限、预算与受保护定义保持。
- 原task check及scope均退出0；T0.2、T0.3原check_frozen均通过。

主要文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `bc0cf0f076e33bf1684f66ece496c6792bc21333c2f9af6e0fbe8ece6d571ac5` |
| docs/TASKS.md | `1981b04f8d4066a8129b616f0710bf01e66248bd59527dec9384ca4f452e1442` |
| workflow.json | `d28b4ef54bb71c7cc3720c90e3233bb9922ca2560a06a2f01750a73ed3d1cb31` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| scripts/tests/test_workflow.py | `596487772cb4f71cdd015c34fc18ed86f6a8bfec76ff57dc4178fee9c9e1378d` |
| docs/PROMPT-START.md | `f72455f542af1f92f63c641a342cbcec286b49479356dd56c444612eb39e1d0a` |
| docs/PROMPT-CARD.md | `62d776ad2ae53a6bd8cc8d774215f4a0b789c9f65a037b478140249ca1d6beed` |
| docs/PROMPT-SUPERVISE.md | `2089bb44078270100a9299d4de55cdee62bb43c3a88a3253ef382abed020f88a` |
| docs/DECISIONS.md | `a29ce606684f4c97a19c973e2c2f3922cb6108b223c920d5a4eb7de19ef1db91` |
| docs/PROGRESS.md | `b5483e5d12b04d99ddaa3cc65dbd8d676dff32085886675729f067d07d839305` |
| docs/reviews/doc-review/round-9.md | `a9b0540b28e32b7942ca6ca8a64d71bee601dccc3753efa0659039687664076c` |
| tests/acceptance/t01.py | `494ed87e7f4b22872423c8fe2e59dc51cc14cb5a042ff505d1daa8b999877635` |
| tests/acceptance/t05.py | `3c8b2446592d604aa29bd53cda6494ebb97bad37009e3ba3bd7d53f5908a4781` |
| evidence/task-state/T0.2/handoff.md | `8a33053a955a9a8d5cceb317c740abfb7b12063664a4e347fdf1aebf290b2a20` |
| evidence/task-state/T0.3/handoff.md | `85b781db473ee00583488b0241a4f09d4918d9c79c09344ff5385292eddd61aa` |

ADR分段SHA-256：

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
| ADR-012 | `6b5c36ac582bfaea4cf97d42f35a2ea515abc903e9b835631469fca4200ed5c0` |

## 授权审查

当前外层提供的人类原话：

> 同意，仅将 T0.2 的开发会话上限从 2 次增至 3 次，让原脚本修复构建产物校验误判，你负责复验，现有权限和资源限制保持不变。

ADR-012及同卡RESOLVED准确保留该决定，范围限于T0.2的一次第三会话。修复目标为已复现的构建产物校验误判及对应回归；开发和验收继续采用ADR-010分工。

当前决定与ADR-011的T0.1例外分别记录，T0.1已使用的第三会话未被转移、重置或扩大。原默认max_sessions=2保持，其他卡沿原上限执行。

## 选卡、暂停恢复与局部参数

已读取原next_card、start、scope_problems和drive参数实现：

- next_card优先返回依赖已满足且有状态的当前在途卡。实测current/next均为T0.3，与先完成T0.3质量门的决定一致。
- T0.2的RESOLVED解除该卡原决定阻塞，但没有强制切换current。
- 当前T0.3完成并形成有效完成证据后，原控制器再按任务顺序选择可执行卡；进入T0.2时由原start处理paused、resume_base及既有范围区段，保留累计会话和失败。
- `--max-sessions 3`只改变本次drive调用的局部maximum，未写回workflow。
- `--once`在一个开发会话形成DONE或WIP后结束本次调用。

因此，先按默认规则完成T0.3，再在原current/next确为T0.2、sessions=2、旧写入者及重命令均退出时，使用唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 3
```

能够落实此次仅T0.2的授权。当前T0.3继续普通`--once`及原上限2；此预算例外无需抢占在途卡、手工切current或修改原控制脚本。

protected保持，原check_frozen通过；此次追加授权通过已有运行参数生效，generation保持0，rebaseline无必要。

## 失败证据与修复必要性

已核对：

- `evidence/runs/20261027T050212121970-t01-build/receipt.json`：finished、exit=1，source与after_source相同，protected与当前一致、generation=0；支撑日志摘要匹配。
- `evidence/runs/20261027T050942522562-t01-negatives/receipt.json`：finished、exit=0，源码前后摘要稳定；支撑日志由原只读verify_evidence复核。
- `evidence/drive/t01-validation-20261027T035813Z/artifact-check-findings.md`。
- 当前`t01.py`的build函数及实际自动化JS制品。

误判因果链得到直接支持：

- `tests/acceptance/t01.py:234`开始的build函数在最终制品检查中扫描`b"/control/next"`。
- 实际`target/t01/frontend-automation/assets/automation-DaVOJhrk.js`存在，SHA-256为`ae1da5efba31bd068ae1f69474bb2f809ce7d03c9e6b3fd15d854aed81c9cbff`。
- 实际制品含`control/next`、`t01_control`，缺少带前导斜杠的`/control/next`。
- 原build日志因此报告`automation frontend mode was not built`，在写入build.json之前退出。

现有记录将9条编译/签名子命令退出0、15个诊断用例通过、整体build失败、准确A1/GUI长测未执行分别保留，未用部分成功代替完整验收。

新增会话用于使制品判断符合真实构建模块和端点，并补接受真实测试产物、拒绝缺失桥和生产泄漏的回归，直接对应已复现问题。生产/测试隔离及严格验收要求保持。

## 原质量门与约束继承

本次授权后仍须从稳定源码重新执行build、准确A1、真实GUI/媒体长测、完整verify、原done/Git及固定提交独立CLEAR。

原35卡、36项验收及其冻结定义保持。TASKS相对round-9基线仅有T0.1正常完成勾选变化，需求、卡正文和验收未变，原protected摘要持续一致。

以下既有约束继续成立：

- 本机macOS arm64必需，其他平台按原ADR保留NOT RUN。
- 原性能指标、单位、样本规模、适用阶段及目标强度保持。
- gpt-6.1-sol、开发workspace-write、巡检read-only保持。
- 单次10800秒、原超时、4096MiB RSS采样、jobs=2、测试线程1和原全局共享锁保持。
- 桌面范围与移动阶段门保持。
- 外层只负责原执行器验收及Git收尾，产品和验收实现交原开发工作流。

## 接续提醒

当前T0.3 handoff已包含ADR-012及“优先完成在途T0.3”的条件。当前T0.2 handoff仍保存旧的“待额外授权”记录；恢复该卡时，应通过原handoff补入本次真实授权、修复范围和一次参数条件。外层已确认会在原next选择、start恢复后更新该交接，保留旧失败历史。

第三会话用完后的任何新增开发需求，继续按原流程报告用户决定，保留原计数、失败与收据。

## 实际检查与未验证项

本轮实际完成：

- 原task check、scope退出0。
- T0.2及T0.3原check_frozen通过。
- 原next实测T0.3；T0.2仍paused、sessions=2、失败4条；T0.3 sessions=1。
- 当前protected、ADR分段及关键文件摘要绑定。
- 原局部max参数、once返回、在途优先及暂停范围恢复语义核对。
- 原build/negatives收据、日志摘要、失败检查实现及真实JS标记核对。
- 已记录driver97261、wrapper97280、CLI97285均已退出。
- 审前、审后源码与受保护定义保持。

本轮未执行修复、build、准确A1、GUI长测或完整verify。T0.2整体build仍为FAIL，准确A1与GUI长测仍NOT RUN；T0.3实现仍待外层真实准备、编译及验收。

未发现本次单卡预算授权与原在途优先规则、暂停恢复、冻结、计数、资源或质量门之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
