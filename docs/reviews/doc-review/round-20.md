# 独立文档复审 round-20

本轮仅审查T0.4一次追加开发会话的授权接续。全程只读，未修改文件、执行测试或变更状态。

## 固定基线

- HEAD：`8ed9f5174226c40c36107af3d961673dc5310b47`。
- source：`7a4517bffdd377357236fa36d34bff1ce27b7c46005144b8b7fbe89d2476c446`。
- protected：`a13052cd3dfcc96cb36c056b2f41fcf31b88dcb3b4ce0b5347ffb4e804781db1`。
- earliest/current/next均为T0.4。
- sessions=2、失败2条、generation=1，hard_sessions为空；默认max_sessions=2保持。
- 工作树干净，check_frozen通过，scope无范围外修改。

实际SHA-256：

| 文件 | SHA-256 |
|---|---|
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `b5f02bcddc18d257684802d494520408b9a5799dae307721242bea9aa806b7ec` |
| docs/DECISIONS.md | `2f02d11659ad1cb2c1034dbe212f28063796675510a57d09142b3bf866364e90` |
| docs/PROGRESS.md | `fdde7f9d78aee2c599ed0e9ae0d8fe88442d300f4589d851aad17f2968097d2b` |
| T0.4 handoff | `3d4292e22890fb8b3cf79285fc908d05edb67a2ce7c6e86212004d5314f312dd` |

ADR-022分段摘要：`f5d9c3f48b001f688a18044d80dccbb75c98cf6a71b8627395e9acc2169b5c57`。

## 授权与执行结论

用户针对仅T0.4追加一次2→3修复会话、原脚本实施/外层复验、权限资源保持的问题，明确回答：

> 批准 T0.4 追加一次

ADR-022、真实RESOLVED及handoff与该来源一致。原两次会话、两条失败和WIP`27b6204e79d240900664ac8fc2f03c4ce85cf419`保留。

紧邻启动再次确认当前卡、sessions=2、旧进程退出及两锁空闲后，唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 3
```

可沿原计数启动第3会话。原局部maximum及once语义保持，其他卡默认预算不变。冻结定义未变，本次无需rebaseline。

## 真实失败与后续质量门

已核验原dependencies/build成功收据，以及A1失败收据`20261028T042514879754-A1`的日志/报告摘要和执行身份。

原Rust日志明确记录：

```text
running 17 tests
test result: FAILED. 11 passed; 6 failed
```

准确A1仍exit1，JUnit为2通过、10失败。五项涉及Host结果的工作协议校验，一项涉及超限错误分类；具体根因由原开发者核实，原schema、反例及17项行为要求保持。

修复后由外层在稳定源码上重新locked构建、准确功能A1、直接完整verify，再原done/Git及固定提交独立CLEAR。ADR-010分工、功能/产品测试分期及严格顺序继续。

4096MiB RSS采样、jobs2、线程1、项目专属锁、原权限及单条/会话时限保持。后卡等待本卡完整闭环。

本轮CLEAR确认预算接续可执行；当前业务仍有6项原生失败，尚未完成。

DOC-REVIEW-VERDICT: CLEAR
