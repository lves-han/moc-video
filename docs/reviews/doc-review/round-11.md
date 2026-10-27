# 独立文档复审 round-11

本轮审查用户授权的项目专属重任务锁、冻结修订与历史完成证据的兼容性。审阅者全程只读，未修改项目、运行drive/run/verify或重命令，未启动子代理。

## 受审基线

- 项目根：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`609075e7aa8f8fc39f93d7bf1ec7daec6e4a15da`。
- 当前源码摘要：`58899d110b87588c71310ad6949d39c947b7ee08fb1486a3e8bdec38fef5b711`。
- 原protected：`bca943a2f9ae74c2f9bdf6d617919b90ae7651a32d246f7788ca4239aaa8e78a`。
- 新protected：`5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481`。
- current及next：T0.2；generation=1，sessions=3，5条失败及原暂停区段保持。
- T0.1、T0.3的原completed结果仍为True。
- 工作树干净，原task check与scope均退出0，当前check_frozen通过。
- 同一heartbeat任务`automation`为PAUSED，等待本轮审查后按当前授权接续。

主要文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `42fb572a8c96808a579807cf3011b23b480a7d6f3ed9c582e7923155f8bab8f4` |
| docs/TASKS.md | `354f51e9ecb5dd235c06268841cf11bd6c19e77e02c792d71835810133bb6826` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| scripts/platform_runtime.py | `6d334927fb80c7f56a261d0483eb42ed3364693a4cb90c60a25ef03bc62c9d39` |
| docs/PROMPT-START.md | `f72455f542af1f92f63c641a342cbcec286b49479356dd56c444612eb39e1d0a` |
| docs/PROMPT-SUPERVISE.md | `2089bb44078270100a9299d4de55cdee62bb43c3a88a3253ef382abed020f88a` |
| docs/DECISIONS.md | `dc79bd5f56bb7e9c9671ee84dcb9c4b20f5a31892ba84c68b448b95347361b12` |
| docs/PROGRESS.md | `701e0d0fdfaf70e97032f882e1aa5e97f173de3f305c39427d01bf2bd62d0e7f` |
| docs/runtime.lock.json | `24aa06b8bcb8cfe4721082424f1e79011a8c7be892805a2ba0e823c8c500828c` |
| tests/acceptance/t00.py | `6a34cfbcf4d23bd08e4ff152aebfa294119fc66c49f31198e32b6c732deff84a` |
| tests/acceptance/t01.py | `afb6b81a5b0bf4b88448352ac70dca3b51760f111f136fcd6a70d1fcf6cbf631` |
| tests/acceptance/test_t01.py | `e85547b50b3f0c3900286e0951d2f13086ba160a6f229d4021751de48b6b4a5a` |
| docs/reviews/doc-review/round-10.md | `6965bf2b00416c5df854a72e59e81225619b564f95ff1a47da9613b1859dedf9` |

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
| ADR-013 | `d2c0c16b73cb276623fa0609ef43d8004ffbe78f4fe876c9211ccf7050294abf` |

## 授权与最小修改结论

当前外层提供的真实人类原话：

> 授权变更，本项目不受共享锁影响

ADR-013明确记录该指令取代此前本项目跨项目共用heavy锁的要求。用户指示的优先级及本项目AGENTS的授权顺序支持此次例外。

提交609075e的实际定义修改仅为：

1. workflow.json的heavy_lock由null改为`/Users/eason/jp/application/evidence/drive/heavy.lock`。
2. ARCHITECTURE C1对应段落同步当前路径、项目内串行及迁移重新绑定要求。
3. 追加真实ADR、BLOCKED/RESOLVED及后续中断记录更正。

原运行控制器、driver.lock、CLI权限、资源、超时、会话计数和任务验收均保持。现有heavy_lock配置直接承载此次需求，未增加控制层或执行器。

## 原锁语义与隔离证据

原`platform_runtime.lock_path(config)`优先使用已配置的绝对heavy_lock，未配置时才采用平台默认路径。原workflow的heavy_path、heavy_active及执行器继续消费这一机制。

当前配置值与原runtime实际解析均为：

```text
/Users/eason/jp/application/evidence/drive/heavy.lock
```

因此本项目run、verify和drive仍通过同一原锁维护项目内重任务串行；其他项目原锁及其任务保持。

外层证据`evidence/drive/lock-change-path-probe.json`记录：

- 新项目锁可acquire并立即释放。
- 原全局锁当时被其他项目占用，acquire=False。
- 两路径不同，protected与generation匹配当前基线。

这项证据直接支持“本项目独立于原跨项目排队”的配置效果。项目间现可并行，单项目原RSS采样、构建并行度及测试线程限制保持。

## 过渡、冻结修订与失败历史

已核对原取消收据：

`evidence/runs/20261027T062936367482-t01-negatives/receipt.json`

其结果为finished、exit=143、raw_exit=-15；尚无payload身份，原日志为空且摘要匹配。记录的执行器90374身份`created:1793082576.374787`已退出。

PROGRESS保留最初exit125表述，并以之后的明确更正记录真实收据及task recover无待恢复项的结果，符合只追加规则。当前5条失败包含此次授权切换导致的中断，未被删除或改记成功。

原rebaseline记录完整：

- 原protected→新protected具有对应真实BLOCKED后RESOLVED及匹配批准摘要。
- generation由0增加为1。
- sessions=3、5条失败、原base、resume_base及暂停区段保持。
- rebaseline_scope仅包含`docs/ARCHITECTURE.md`和`workflow.json`。
- 当前check_frozen、scope及next读取均成立。

原rebaseline还会检查其他已开始未完成卡、未完成重命令、活跃驱动及范围外修改。当前T0.1/T0.3均由原completed返回True，T0.2为当前唯一未完成的已开始卡，流程与其前提一致。

## 历史runtime.lock与已完成卡

docs/runtime.lock.json相对T0.1完成提交保持，包含T0.1当时观察到的旧global_heavy_lock、旧基线与对应历史绑定。保留这些原始观测符合历史证据完整性；ADR-013与当前C1明确将现行执行路径交给workflow配置及原runtime解析。

已核对以下影响：

- 原completed根据已完成卡的受验提交、历史source/protected/generation及对应门槛检查证据。
- T0.1、T0.3的完成收据及固定提交巡检保持，原completed实际仍为True。
- T0.2原completion_evidence严格核对当前source、protected及generation；generation0旧build/验收收据无法满足本卡generation1完成检查。
- 当前T01具有独立resource_receipt，从现行配置核对原资源包装与预算；完整verify入口也未调用旧T00 A1。

执行提醒：旧T00验收中的resource_scope会严格比较当时global_heavy_lock，因此若以后将该旧A1直接放到新配置下重新运行，会报告绑定变化。它应按相应受验提交复现；当前专属锁的运行事实由新配置、新收据和ADR-013记录。当前T0.2验收路径与历史T00数据分别使用，未发现本次接续受其阻断。

## 新基线验收闭环

当前generation1尚无业务运行或完整verify收据。修复WIP06764f2及旧build成功记录继续保留，新的完成证据应依次取得：

1. 当前稳定源码下重新build，生成与新source/protected绑定的产物记录。
2. 19个回归用例实际执行。
3. 准确A1，包含原真实GUI/媒体长测和生命周期要求。
4. 直接执行完整verify。
5. 原done、受验完成提交及固定提交独立CLEAR。

原A1及完整verify的source/after_source、protected、spec、generation、准确argv、JUnit cases、日志和报告摘要核验均保持。

T0.2第三开发会话已使用；当前接续为外层验收。后续新增开发需求继续沿原预算和用户决定流程处理。其他新卡默认上限仍为2。

## 需求与运行约束继承

此次定义变化集中于锁路径和C1对应说明。SPEC、任务正文/验收、原控制脚本、平台范围、性能指标及数据契约保持。

既有35卡/36项验收、macOS arm64必需目标、移动阶段门、原指标强度、Docker真实集群成员/容器限额与宿主RSS合计核算及自产清理要求继续。

gpt-6.1-sol、开发workspace-write、巡检read-only、4096MiB macOS RSS采样、jobs=2、测试线程1、单次时长、命令超时及失败规则均保持。项目根及专属锁使用绝对路径，迁移时按C1重新绑定和审查。

## 实际检查与未验证项

审阅者实际完成：

- 原task check、scope退出0。
- 当前check_frozen通过；current/next=T0.2。
- 原runtime锁路径与配置值一致。
- 阅读原lock_path、heavy_path/heavy_active、rebaseline、completed及completion_evidence语义。
- 核对取消收据、日志摘要、执行器真实退出身份及更正记录。
- 核对generation1、sessions3、5条失败及原暂停区段。
- 原T0.1/T0.3 completed均True。
- 核对主要文件、全部ADR分段、最小提交差异与旧runtime.lock历史保持。
- 核对外层新锁隔离探测。
- 核对外层`lock-change-validation.json`：PASS，116项、109通过、7平台条件跳过，无failure/error；完整日志为`evidence/drive/lock-change-validation.log`。
- 当前generation1运行列表及完整verify列表为空。

本轮未执行锁竞争测试、业务回归、build、GUI长测或完整verify。新基线下T0.2质量门仍待实际执行，旧成功证据保留各自原始证明范围。

未发现此次真实授权、专属锁配置、冻结修订及后续验收闭环之间的阻断问题。

DOC-REVIEW-VERDICT: CLEAR
