# 独立只读范围提案审查 round-21

本轮仅审查待批准的T0.5范围补充及既有基线迁移路线。全程只读，未修改文件、执行测试或启动开发。

## 现行基线

- HEAD：`8fbd05ddc63a80a0a0b7f980a70f3e778afa3007`。
- source：`67416fd5cb32050b9501fe81272479ba8f22351aa69d3d5f92c309f5aa32b16f`。
- protected：`a13052cd3dfcc96cb36c056b2f41fcf31b88dcb3b4ce0b5347ffb4e804781db1`。
- current及最早未完成卡为T0.5，sessions=0、generation=0、失败0。
- T0.13保持paused，sessions=1、generation=1、失败0，共用上述旧protected。
- 原T0.1至T0.4 completed均成立，工作树干净。
- 当前T0.5因范围待决定BLOCKED，原next为T0.19；严格顺序继续停在T0.5。

实际SHA-256：

| 文件 | SHA-256 |
|---|---|
| 现行docs/TASKS.md | `29c706890bb05b6e423ce81d51507a5d9523c5258bc7da8d3d413a482431ade3` |
| src-tauri/src/host.rs | `41a2b9be73992c015dd8bb241010b33536dd12cee10ed65d44cf2c5e04275eb9` |
| proposal.md | `64b3882fc320181d6be6294fdcd031f3fe11be5c80ee34dcd40f046d6fce3de0` |
| TASKS-candidate.md | `460dd08690e5191be0373a1484334ccb372ada2adfc7dd9078bd62236aa02648` |
| scope.patch | `67d340c1d0359cf9d694e40b757d8d40e9fed9d9a25a6e470b50a72b46c8b91a` |

提案文件位于`evidence/drive/n01-scope-proposal/`。

## 范围必要性与最小性

已读取实际Host及命令入口：

- Stremio同步分支固定返回ENGINE_NOT_READY。
- Stremio内容分支同样返回占位错误，resolve仍明确拒绝。
- committed、Call.job及提交锁由host模块持有。
- commands.rs通过Host.admit/execute处理业务，来源注册表提供快照及发布能力。

因此，新增Stremio适配器后仍须在原Host接线，才能复用现有取消、配置世代、同步序号、提交锁及输出校验。仅补host.rs是直接且局部的路径。现有MacCMS公开HTTP能力亦可复用，未发现本提案需要另建宿主状态或扩大其他文件范围。

候选仅在T0.5范围中增加host.rs；准确checks、cases、依赖、义务和预算保持。只读预算解析无问题，patch适用性检查通过。

## 合法迁移路线

当前全部已开始未完成卡为T0.5和paused T0.13，正好对应既有批次入口的当前卡及暂停集合。

取得真实范围和相应迁移批准后，可追加准确BLOCKED/RESOLVED、批准摘要及暂停集合，通过既有：

```text
task.py rebaseline T0.5 --protected <新摘要> --include-paused T0.13
```

完成迁移。入口已支持持有两锁、完整前置核验、历史/计数/区段/paused/current保持及旧证据失效，无需再次修改控制器。

T0.13仅迁移元数据，顺序BLOCKED及隔离WIP继续保留；T0.5仍默认2会话。迁移后须对固定实际内容独立复审，再按严格顺序启动开发。

## 验证范围与结论

已核对原预检收据`20261028T060930376954-n01-scope-proposal-check`退出0、候选35卡及范围/checks/deps/预算保持。审阅者未重新运行测试。

本报告确认提案可交用户审批。实际冻结文件、新protected、迁移状态及Stremio业务实现均尚未生效或验证，业务验收仍NOT RUN。

DOC-REVIEW-VERDICT: CLEAR
