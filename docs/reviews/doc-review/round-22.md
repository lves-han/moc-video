# 实际范围与基线迁移独立复审 round-22

本轮全程只读，未修改文件、状态或启动测试、开发。

## 固定基线

- HEAD：`a94ea8e42e51929e7f51f6a7f158ac1586f8a0fc`。
- source：`67416fd5cb32050b9501fe81272479ba8f22351aa69d3d5f92c309f5aa32b16f`，保持未变。
- 新protected：`01b1775adb18066ee1627c284689f16eab9f1cb8c4e734bee4ed887660aac45d`。
- earliest/current/next均为T0.5，工作树干净，check_frozen及scope核对成立。
- ADR-023分段摘要：`f18853d93cb1d48819e9e84d9c3185f94d80bac46b42e99f9f6244692b277478`。

实际SHA-256：

| 文件 | SHA-256 |
|---|---|
| docs/TASKS.md | `460dd08690e5191be0373a1484334ccb372ada2adfc7dd9078bd62236aa02648` |
| docs/DECISIONS.md | `aac26046dddd23898cb530e941511449209ec9ae91612e102e46ddcaae06c3db` |
| docs/PROGRESS.md | `0b95bfc533d6a44dd9eaac8e3334ad0963db4cf440395f420839139b6ea531e0` |
| scripts/workflow.py | `b5f02bcddc18d257684802d494520408b9a5799dae307721242bea9aa806b7ec` |
| T0.5 handoff | `0e10452d0ca22f7d86d38fb89146a74cf25c014e43d92d48a711f3a6cfee7cf1` |

## 授权及实际迁移

用户针对T0.5补host.rs、联动迁移T0.13暂停基线并保留历史、暂停及默认2次预算的问题，明确回答：

> 批准 T0.5 补入 host.rs

ADR-023及真实RESOLVED、批准摘要、暂停集合与该来源一致。实际TASKS与round-21单行候选逐字节一致，仅增加本卡host.rs范围，原义务、checks、cases、依赖和预算保持。

已读取迁移前后记录并逐字段核对：

| 卡 | generation | sessions | 失败 | paused |
|---|---|---:|---:|---|
| T0.5 | 0→1 | 0 | 0 | false |
| T0.13 | 1→2 | 1 | 0 | true |

原base、resume_base、segments、last_note、会话、失败和paused均保持。两卡记录相同批准事件及迁移集合。T0.13顺序BLOCKED与隔离WIP保留；原T0.1至T0.4 completed仍成立。

迁移使用既有原入口，控制器未变，无新产品代码。原入口的持锁、前置校验、历史保持和generation失效语义继续适用。

## 接续与未验证项

T0.5保持默认2次开发预算、原权限、4096MiB RSS采样、jobs2、线程1、项目锁及单条时限。ADR-010分工、严格顺序及功能/产品测试分期继续。

固定文档与实际迁移可接续；完成启动前身份和锁核对后，可用普通`drive.py --once`启动T0.5第1会话。

Stremio业务实现及准确A1仍NOT RUN，本报告未验证业务通过。

DOC-REVIEW-VERDICT: CLEAR
