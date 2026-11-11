# 固定公共 DNS 修订独立文档审查

## 受审基线

- 本次审查覆盖 ADR-049、ARCHITECTURE C6 和 T0.19 补充验收，相较 round-42 的有限变更。
- protected：`5f24a743a02cebf238fccdc755a1f5c8ae2aecd3617d3fc2d240d577554de7d1`。
- ADR-049 摘要：`af60be8e05e3968e605ccad0552d56ec96540b66e1529fc1337b94b43d54e601`。
- 当前卡 T0.19，generation=2，sessions=1；旧失败保留，blocked 为空。
- 未提交受审文件内容摘要：
  - ARCHITECTURE：`6bba30704558c6897343b6bed94e25147401f5818c80a19b8cf1e3399fd376ee`
  - TASKS：`6fc2385ba59e24c53ef207c9086f271af6c56706c7987a9063a755f66c6891cd`
  - DECISIONS：`8a5c77ede035cb38556a7c336cf506bb9cda50711e08f2428b0503c1afc46dbc`

## 覆盖与可行性结论

未发现本次有限修订的阻断问题。

用户明确指定 `1.1.1.1`、`8.8.8.8`，并要求不使用本机 DNS。ADR-049及C6据此固定公共解析服务器，明确排除系统DNS、hosts、搜索域和失败后的本机回退；IP字面量仍执行地址校验。复用既有hickory依赖且不新增设置界面，符合当前最小范围。

C6继续覆盖API、图片、媒体、字幕、HLS子资源和每次重定向。全部A/AAAA结果的公网校验、混合内网结果拒绝、连接地址绑定、实际peer核对、Host/TLS/SNI与证书验证均保留。Fake-IP继续拒绝。明文DNS被网络劫持的可能已准确披露。

T0.19将该行为纳入现有A1及R12/R49/REQ-T02，明确要求受控DNS验证双栈、单服务器失败转移、双服务器失败、取消和超时，以及Fake-IP、混合地址、重绑定、重定向和peer负例；真实公共DNS的JSON及媒体链路另行验证，失败如实记录。错误实现中的系统回退、内网放行及失效地址复用均有对应反证要求。

实现可在既有公共解析入口局部修改，范围包含src-tauri与tests，无后卡前置或新依赖。原T0.19真实媒体闭环及后续逐卡顺序保持；其他平台NOT RUN、移动阶段门和ADR-044性能强度未变化。公共DNS实际可达性尚待实现后的真实网络验证，当前无已确认的新环境阻断。

## 实际验证

- `task.py status`：protected、ADR-049摘要与generation=2一致。
- `task.py check`：35卡通过。
- `task.py next`：T0.19。
- T0.19范围外修改为空，blocked为空。
- SPEC与归档逐字节一致。
- 核对现有DNS/peer/重绑定回归、公共HTTP入口、workflow预算及round-42原审查结论。
- 本轮保持只读，未修改文件、运行业务测试或启动开发。

## 未验证项

固定公共DNS实现、故障转移、真实DNS网络请求、媒体播放、A1、完整verify及代码独立巡检均为 **NOT RUN**。本结论仅确认修订规划充分。

DOC-REVIEW-VERDICT: CLEAR
