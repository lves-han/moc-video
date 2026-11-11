# 独立文档复审 round-23

本轮仅审查T0.5一次预算接续及既有DNS证据。全程只读，未修改文件、执行测试、发起网络请求或启动开发。

## 固定基线

- HEAD：`bfca5ce2e75116129f9d4d9307018c059e65d7ab`。
- source：`81c9bab9d5fa26bd0824f34774688b6afa54ebdc5cb8cbcddab749278aa69476`。
- protected：`01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d`。
- earliest/current/next均为T0.5；sessions=2、失败3条、generation=1，hard_sessions为空。
- 默认max_sessions=2保持，工作树干净，check_frozen及scope核对成立。
- ADR-024摘要：`038231a74a95bd84aafcedbe60ef46676b9f1fc63329bea5aff62a515c5a49cd`。

实际SHA-256：

| 文件 | SHA-256 |
|---|---|
| docs/DECISIONS.md | `6a69149f2dc8c956b2abc31c0a11c453b96d98e0ae99ba77929359fe30e0ebc5` |
| docs/PROGRESS.md | `417df1891cb8bbf03b2e0f137019a4ae3a0b023da5c748f319a0c4d15de1fcdc` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| T0.5 handoff | `3713da26fedde139261e758f5087ab3623b4462ff893f911280e676eb527d017` |

## 授权与接续

用户针对仅本卡追加一次2→3、原CLI修复并自验证、外层复验且权限资源保持的问题，明确回答：

> 批准 T0.5 追加一次

ADR-024、真实RESOLVED及handoff与该来源一致。第三次尚未消费；原两次会话、三条失败、WIP及原报告保持。

安全前提再次成立后，唯一一次：

```text
/Users/eason/jp/application/.venv/bin/python -B scripts/drive.py --once --max-sessions 3
```

可沿原计数启动第3会话。原局部参数及once语义保持，其他卡默认预算不变。冻结定义未变，本次无需rebaseline。

## 失败与DNS事实

已核对原A1及日志/报告摘要。原Rust日志明确为19项实跑、10通过、9失败；准确A1仍exit1，JUnit为2通过、15失败。

原schema的DubStatus仅允许`dubbed/subbed/unspecified`。原受控枚举诊断确认`groups[].dub_status="unknown"`非法、`unspecified`合法。修复应保持其他字段合法unknown语义、原schema、19项行为及真实语言门。

用户另答“已调整，可重新核对”。原DNS复核`20261028T093539289489-n01-dns-recheck`退出0，系统及默认resolv.conf的A记录均为：

- `104.26.8.49`
- `104.26.9.49`
- `172.67.74.162`

均记录为公网单播，AAAA无记录。本轮读取并核验原日志摘要，未重新查询。该结果证明当次解析条件；生产HTTPS、TLS/实际peer、扩展部署版本及同ID中英文响应仍待原A1验证。公网安全策略保持。

原A1及DNS执行器、payload记录身份均已退出。

## 保持的质量门

ADR-010开发/外层代验分工、严格卡序及功能/产品分期继续。原4096MiB RSS采样、jobs2、线程1、项目锁、权限及时限保持。

修复后应重新locked构建、准确A1、直接完整verify，再原done/Git及固定提交独立CLEAR；后卡等待。

本轮CLEAR确认预算接续可执行。当前业务仍有9项原生失败，真实语言验收尚未完成。

DOC-REVIEW-VERDICT: CLEAR
