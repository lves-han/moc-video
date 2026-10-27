# 独立文档复审 round-12

本轮审查T0.2第四会话授权、严格卡序、越序WIP的可逆隔离，以及T0.4已批准范围的后续冻结路线。审阅者全程只读，未修改项目、运行drive/run/verify或重命令，未启动子代理。

## 受审基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`7ec98bd36c9488aabb677a0630677385f79e0ae3`。
- 源码摘要：`58899d110b87588c71310ad6949d39c947b7ee08fb1486a3e8bdec38fef5b711`。
- protected：`5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481`。
- 最早未完成卡、current、原next均为T0.2。
- T0.2：sessions=3、失败6条、generation=1，hard_sessions为空；最新note在最后一次失败之后。
- T0.4：paused、sessions=1、失败0；T0.13：paused、sessions=1、失败0。
- workflow默认max_sessions=2保持；原task check、scope及T0.2 check_frozen通过。
- 工作树干净，受保护定义与round-11一致。

主要文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `42fb572a8c96808a579807cf3011b23b480a7d6f3ed9c582e7923155f8bab8f4` |
| docs/TASKS.md | `354f51e9ecb5dd235c06268841cf11bd6c19e77e02c792d71835810133bb6826` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| docs/DECISIONS.md | `17d3d11f7b896c78d61ef10db2746d3a3eb71d8467e581d3742a2ba3ba1019a7` |
| docs/PROGRESS.md | `be0430b07a7d36246bafe5de7c7b905c5295d61f708d90b80404e73cbddf517e` |
| docs/reviews/doc-review/round-11.md | `bd3524f6d4cde8f71c2843281cf9a9b9f6a48d15fb77e16f00facc6119b9e4c7` |
| tests/acceptance/t01.py | `afb6b81a5b0bf4b88448352ac70dca3b51760f111f136fcd6a70d1fcf6cbf631` |
| tests/acceptance/test_t01.py | `e85547b50b3f0c3900286e0951d2f13086ba160a6f229d4021751de48b6b4a5a` |
| evidence/task-state/T0.2/handoff.md | `40d48a11d814469c99bfab6f271729587ff3f1d30f297d9578039d394c38db4f` |

ADR-001至ADR-013的原分段摘要已逐项核对，全部与上述round-11报告一致。本轮新增分段为：

| ADR | SHA-256 |
|---|---|
| ADR-014 | `ef2a5363e382e5ddd1f085a1f7ae8671e9e420ed2eca8f565d6a5ef58ec78017` |
| ADR-015 | `22efa652b4d7491e68730f18d581764a679f8ed76baed2b673cd588ee93e7d48` |
| ADR-016 | `7bafb564f37f4c8f97dc2dd5984b1d566f621a0b96456e4c93fbf55f504862e8` |

## 当前T0.2授权与可执行性

外层提供的真实人类原话：

> 同意，仅将 T0.2 的开发会话上限从 3 次增至 4 次，让原脚本修复窗口尺寸和 JUnit 失败报告生成问题，你负责复验，现有权限和资源限制保持不变。

ADR-014、真实RESOLVED及当前handoff准确保留该决定。范围限于T0.2一次第四会话，修复两个已复现问题及对应回归，原权限、资源、单次时长与质量门保持。

原drive的局部maximum覆盖及`--once`语义保持。当前满足next/current=T0.2、sessions=3、原冻结检查通过的前提；确认旧写入者和重命令退出后，唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 4
```

可沿原状态启动T0.2第4会话。该参数仅作用本次调用，默认配置仍为2。原3次会话、6条失败与generation=1保留，无需rebaseline。

当前T0.2接续未发现阻断问题。

## 严格卡序与Git隔离

真实人类指令：

> 前一任务未完成或阻塞，禁止开启下一个任务

ADR-015明确覆盖旧独立分支推进方式。外层在原next之外检查TASKS文件顺序的最早未完成卡，并要求准确验收、完整verify、完成提交、固定提交独立CLEAR及干净工作树全部成立后才推进。该顺序检查与原`--once`组合可保留控制器原实现，同时执行最新人类顺序要求。

已核对隔离历史：

- T0.13完整WIP保留于`b5f979e`。
- `ead5af1`仅逆转该WIP的产品/测试文件差异；原PROGRESS内容及Git历史保留。
- 当前原snapshot与`b5f979e^`的snapshot完全相同，均为`58899d110b87588c71310ad6949d39c947b7ee08fb1486a3e8bdec38fef5b711`。
- WIP中的16个产品/测试路径与隔离提交对应，未见额外源码改动。
- T0.13原session1及暂停状态保留，按真实顺序要求BLOCKED等待前卡。
- 已记录driver33018及CLI33076均已退出。
- 原next/start已回到T0.2，当前最早未完成卡检查一致。

该操作保留越序实现，撤销其对当前复验源码的影响，未改写历史或重置任务计数。将来按序恢复T0.13时，仍须在原卡范围内恢复保存WIP、处理届时实际冲突并完整验证。

顺序核对须继续包含固定提交独立CLEAR；原completed函数本身主要核验完成提交和验收证据，外层应同时核对独立巡检结果。

## 两项修复的真实证据

已核对原A1收据：

`evidence/runs/20261027T064841885921-A1/receipt.json`

其exit=1、source/after_source均与当前源码相同、protected及generation=1匹配，日志摘要有效。

原日志及native-findings支持以下因果链：

- 真实应用握手通过，native物理尺寸2400×1600、scale=2，WebView viewport为1200×768。
- 当前断言要求viewport恰为1200×800，因32px差异报告`default window size differs`。
- 随后的JUnit保存对无actual字段的失败summary访问`row["actual"]`，触发KeyError；原控制器继续保持FAIL并报告JUnit缺失。
- 真实应用失败清理与fixture资源释放事实分别保留；两种30分钟播放及20生命周期尚未执行。

第四会话应从实际窗口/内容区语义定位尺寸因果，保留准确布局断言；JUnit修复应保留原失败详情及汇总，补失败报告回归。授权未降低真实GUI、媒体时长、生产/测试隔离、资源和完成门。

## T0.4后续冻结路线约束

“批准补入 host.rs”已由ADR-016真实记录，具体范围批准保持有效，后续无需重复确认这一文件范围。当前TASKS尚未修改，本轮亦未把T0.4标记为已解锁。

原rebaseline函数会拒绝存在其他已开始未完成卡的情形，并将paused卡计入检查。当前对T0.4而言，其他未完成卡为T0.2和T0.13；T0.2完成后，暂停的T0.13仍会触发该限制。

因此，实际落实T0.4范围补充时，存在明确的后续流程约束：

- 保留当前T0.2优先修复及完整验收。
- 到达T0.4时，保留已批准的host.rs范围，核对原rebaseline拒绝条件并报告。
- 需要另行决定的是多在途卡冻结基线的有审计迁移路线，例如针对当前T0.4及受影响暂停T0.13的原控制器最小能力修订方案，完整保留计数、失败、区段和旧证据并独立复审。
- 原流程没有现成的多卡联合rebaseline入口；当前真实范围批准不应被扩写成已批准控制器变更。
- 严格顺序要求继续有效，后续路线不得通过提前完成T0.13、手写状态、假完成或删除原会话来处理。

该后续约束已在ADR-016保留，未妨碍当前受保护定义保持不变的T0.2第四会话。T0.4实际改范围及基线迁移仍待后续单独处理。

## 约束继承与实际检查

当前SPEC、ARCHITECTURE、TASKS、workflow和原控制器与round-11定义相同。原macOS arm64必需范围、性能指标、真实集群Docker/资源核算、专属项目锁、4096MiB RSS采样、jobs2、线程1、CLI权限及时长均保持。原控制验证116项、109通过、7平台跳过证据继续有效。

本轮实际完成：

- 原task check、scope退出0，check_frozen通过。
- 最早未完成卡/current/next均为T0.2。
- 原sessions3、失败6条、generation1、默认2及hard_sessions为空的核对。
- 当前源码与保留WIP父提交snapshot一致性核对。
- WIP与逆补丁隔离文件列表、历史保留及旧进程退出身份核对。
- 原A1收据、失败日志摘要、native-findings及JUnit异常核对。
- ADR-001..013摘要延续、新增ADR-014..016及关键文件绑定。
- 原rebaseline其他未完成卡限制的源码及实际状态核对。

本轮未执行代码修复、构建、回归、准确A1或完整verify。当前A1仍为FAIL，GUI长测尚未完成；新第四会话结束后须从稳定源码重新取得全部原质量门证据。

本轮结论确认本次预算、严格顺序、可逆隔离与当前T0.2接续可执行；T0.4实际冻结范围修改的迁移路线继续保留为后续待决定事项。

DOC-REVIEW-VERDICT: CLEAR
