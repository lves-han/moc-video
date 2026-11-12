# 播放服务合并后文档独立复审 — round-58

审查日期：2026-11-11
集成基线：远端 main `bca68af7f5a36f24f4cbee64c641cc38da22c305`。
审查位置：本次文档同步的临时克隆；原开发工作区及未提交应用改动继续保留。

## 受审基线

| 文件 | SHA256 |
|---|---|
| `docs/FOLLOWUP-PLAYBACK-SERVICE.md` | `4e97ec7fd9b1fd26fed3ab138063f7c2ce9f816e7266242b86c69e18ed2b20bd` |
| `docs/TASKS.md` | `9b0ad36e068864238d2c39b2c53c0329376459d19e27b155f4ff9486aa3961e2` |
| `docs/DECISIONS.md` | `830180744496044ad3d3a7fe5fdb3d9ab34b71236551aa4d5edafccbe09c2dff` |
| `docs/ARCHITECTURE.md` | `20fff12e3c02e4b21ed6c11bad785a33df203832add2d49f81624b4561d477d6` |
| `docs/BACKLOG.md` | `84ab8a7b623a9f20beab34b6d6fc68ebbbeea260f17c61957f41c7331e3792f5` |
| `docs/FOLLOWUP-AUTO-PLAYBACK-SOURCES.md` | `746d9cf93a126ed5a6cb565988559c9790f6dedf1adee35b00b74180da77cf9b` |

- `protected`：`f0cb326abf5b9501c976263fcb8983cdae8429a23dbc75d2d96e847982d54d07`
- ADR-078 分段摘要：`ac35750afc3ba7a4b28ba185458d8ee5dd360ed9e4af3dc1941cdfafc6920f8d`

## 复核结论

合并保留远端 ADR-068–ADR-074 的完整正文。播放服务四项决定顺延为 ADR-075–ADR-078，方案、任务覆盖、架构与历史方案中的相关引用已经同步，ADR 标题编号唯一。

网络约定与远端 ADR-068 一致：来源、图片、媒体、字幕和 HLS 子资源允许内网、回环及 Fake-IP，继续保留固定 DNS、TLS、协议检查、跨 origin 凭据处理与资源上限。服务令牌请求另行限定到配置的 origin 和基础路径；局域网服务域名可使用系统 DNS。方案、架构、ADR-078 与 T0.47 的验收均采用该规则，原先针对私网媒体的拒绝要求已经更新。

其余八项修订继续完整保留：当前 TMDB/Stremio 记录身份、类型与尾年匹配、等待测速、候选及线路生命周期、自定义 Stremio 元数据第一期交付、播放 ID 转换、首次首页快照、令牌持久化和日志脱敏。各项仍有对应任务与代表性检查。

三期路线及个人娱乐应用验收安排保持一致。T0.47 保留授权阶段门，`authorized(T0.47)=false`，T0.48、T0.49 依次依赖前卡。文档同步与推送安排未开启业务开发。

本次受审范围未发现需要继续修订的实质问题。

## 实际检查与未验证项

实际完成：

- 核对六份文档 SHA256、`protected` 和 ADR-078 分段摘要。
- 字节比较确认远端 DECISIONS 完整前缀保留。
- 检查 78 个 ADR 标题编号唯一，两个方案的 ADR 引用均能对应现有编号。
- 路线结构检查通过：54 张卡，三张服务卡均未完成，阶段门关闭。
- `git diff --check` 通过；当前已跟踪差异及新增文件均在文档目录。

本轮为独立只读审查。未修改文件、启动开发、构建或运行业务验收；服务实现、第三方兼容性与播放表现仍为 **NOT RUN**。round-57 作为合并前文档审查历史保留，本报告对应上述合并后基线。

DOC-REVIEW-VERDICT: CLEAR
