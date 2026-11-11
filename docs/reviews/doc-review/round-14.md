# 独立文档复审 round-14

本轮审查T0.2一次第6开发会话授权及补充真实播放诊断编排。审阅者全程只读，未修改文件、运行drive/run/verify或重命令，未启动子代理。

## 受审基线

- 项目：`/Users/eason/jp/application`。
- 控制解释器：`/Users/eason/jp/application/.venv/bin/python`。
- HEAD：`e315d0cb6f1bf3a5d166beba0510fe0194882566`。
- source：`af4d62919c2b6ddc031e4fda0176cccdc60171e205b5d5c506d0f10406503297`。
- protected：`5d991a18504cf367995c35733d2d3ed0dc16a94068affb3dedeb15b3f8921481`。
- earliest/current/next均为T0.2；sessions=5、失败8条、generation=1，hard_sessions为空。
- workflow默认max_sessions=2保持；工作树干净，原task check、scope及check_frozen通过。
- 新增ADR-018分段SHA-256：`3afac14679cd0259aa796032d3dc17c98a9915bd1eaadb2f80f2c19cd2ab22c4`。

关键文件SHA-256：

| 文件 | SHA-256 |
|---|---|
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `083776a0242feb18d4d9825804c0d1f97e845fc5b978e89a8a577686b7d05057` |
| docs/DECISIONS.md | `68d72f6fda939cb3d7c538ac0be49f54935a694f30fe2e7cc64266dc519e79a2` |
| docs/PROGRESS.md | `a7a2096ca9b77376df6289183ba46153370f41eca35a5ab6612170e5e8358cac` |
| docs/reviews/doc-review/round-13.md | `04a6bc51457635c06c9dd70ba3e5e523804ed99f2f643ea99b01b3831a597fc0` |
| tests/acceptance/t01.py | `cbf227108cf7d245a72badf4641424808c0630446f92e4c0c8f1ae67fdaeaa9f` |
| tests/acceptance/test_t01.py | `23ca6e27c9251b8d30a7e4a90f6064e51da43d2f1a4455f512662f197174c909` |
| tests/fixtures/t01/automation.ts | `0fe42b6e7f039d957db1341bd525ffd3c2a3bd3d8ee3db32ae9f531d4f6ac859` |
| tests/t01-automation.test.ts | `cec4dbe615c6520941017f91e0f69104e70d440b58d0fe7a8c61f1f5ff2e70c1` |
| evidence/task-state/T0.2/handoff.md | `6925b2c59aa22161e1c38042177a16ba2a2a7c3bd564cc70a48940a6d4691a82b4` |

## 授权与一次参数结论

ADR-018保留完整询问：仅T0.2追加一次5→6修复会话，修复Node装载冲突并继续真实长播诊断，原脚本实现、外层复验、权限资源保持。人类针对该问题明确回答：

> 批准 T0.2 追加一次

真实RESOLVED及handoff与该来源一致。此前转去回答用户问题后，当前sessions仍为5，此例外尚未消费。

原局部maximum与`--once`语义保持。在本轮CLEAR且补充诊断及其全部资源退出后，紧邻启动再次核对earliest/current/next=T0.2、sessions=5，唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 6
```

可沿原状态启动第6会话。原计数、失败及区段保持，默认上限仍为2；protected未变，本次无需rebaseline。

## 失败因果证据

已核对以下原收据及支撑摘要：

- `20261027T092323112360-t01-build`：exit0。
- `20261027T092359691039-t01-negatives`：exit1。
- `20261027T092712049948-t01-cjs-import-diagnostic`：exit0。

三次运行source/after_source相同，protected及generation=1匹配；日志/报告摘要有效，原执行器及payload均已退出。

静态转译诊断直接显示：

```text
5: const player_1 = require("../../../src/player");
7: function require(value, message) { ... }
```

同词法作用域的断言函数经提升遮蔽VM提供的模块加载器，返回undefined后导致nativeInvoke访问失败。该证据支持新增Node回归装载冲突的修复范围；Python30项通过与Node10项失败分别保留。

真实MP4长播停滞的根因仍待运行证据确定，未将该装载错误等同于真实Vite应用的长播根因。

## 补充真实诊断编排

已读取当前原收据`20261027T094322135992-t01-native-stall-diagnostic`及其payload。该运行目前为running，绑定T0.2/session5、当前protected和generation1，source保持稳定。

payload仅编排现有：

- resource_receipt及check_build；
- Fixture、NativeSession；
- identity/security/open/controls；
- long(seconds=1800)及validate_long；
- 原session/fixture清理，并在finally独立保存native-diagnostic.json。

SIGTERM/SIGINT转换为异常以进入原清理路径。未见修改产品/回归实现、替换既有断言、模拟真实结果或改写准确A1门的操作。

这是ADR-010外层验收职责内的补充诊断。其原运行ID、单独输出路径和diagnostic_only标记与准确A1区分明确。Node回归失败仍保留；即使该MP4诊断通过，完整回归、HLS、20生命周期及准确A1等仍须按原门执行。

第6开发会话应等待该重命令及应用/WebKit/fixture全部退出，再读取新results/samples、诊断结果及原收据，更新真实note和handoff。若补充诊断新增失败，原失败历史按实际追加，不以本报告观察到的8条固定覆盖。

## 完整质量门与约束继承

修复后从稳定源码重新执行原build、全部回归、准确A1，真实MP4/HLS各1800秒及20生命周期保持；全部通过后直接完整verify，再原done、受验提交及固定提交独立CLEAR。

ADR-015严格顺序、ADR-013项目专属锁、ADR-010分工继续。gpt-6.1-sol、workspace-write/read-only、4096MiB RSS采样、jobs2、线程1及原时长保持。当前仅恢复T0.2。

T0.4已批准host.rs范围及原多暂停卡rebaseline限制继续沿round-12保留，本轮没有控制器扩展授权。受保护定义未变，既往控制验证保留其原证据范围。

## 实际检查与未验证项

本轮实际完成原task check、scope、check_frozen、最早未完成卡/current/next、计数/预算字段、原收据及摘要、转译因果证据、补充诊断payload和当前运行状态核对。

本轮未执行修复、测试、长播或完整verify。补充诊断正在运行，尚无完成结论；本版Node回归仍为FAIL，本版准确A1尚未执行。真实停滞的原因及修复效果待现场诊断与后续原开发/复验闭环。

未发现此次单卡预算授权、补充诊断与原质量门之间的阻断问题；第6会话启动前须满足诊断退出及原身份/锁核对条件。

DOC-REVIEW-VERDICT: CLEAR
