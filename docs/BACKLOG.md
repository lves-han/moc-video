# 墨客视频 停车场

仅追加：UTC日期、发现于哪张卡、问题一句话、相关文件。记录不授权偏离当前任务。


## T0.27 命令清单静态状态文案维护 2026-10-28T03:48:09.773767+00:00

- 来源：T0.2固定提交独立CLEAR中的低优先级建议。packaging/t01-commands.json的status仍为运行前pending描述；本卡功能实际已完成，原handoff现已同步。后续T0.27在其packaging范围维护时将静态文案改为管线用途说明，避免与原任务状态混淆；当时随本卡当前源码/质量门验证。原受验完成源码保持，实际状态由原task与收据给出。


## 后续控制器维护：受限异常退出历史日志一致性 2026-10-28T06:06:33.006164+00:00

- 来源：T0.4独立CLEAR报告的中等历史证据缺口。n00-compile受限run记录日志SHA与当前实际日志SHA不同，原始记录保留；capture及副本见当前T0.4验证目录historical-hash-audit。执行器报sysctl EPERM后日志还有Cargo输出，迟到输出/异常清理原因尚未复现。当前成功验收与完整verify均匹配原摘要，独立巡检CLEAR。后续只读复现并核对platform_runtime.py异常路径；若需修改冻结控制器，先走真实批准/原基线/独立复审，避免重写旧收据伪修复历史。

## 执行器异常清理P1证据补充 2026-10-29T02:22:32.259657+00:00

T0.6原固定巡检20261029T021055842631发现与旧N00同类日志后写问题，现代码/日志链已确认初始枚举与清理枚举EPERM导致漏登记/漏wait并提前finished。严重度P1，当前T06 BLOCKED、撤回勾选并暂停后卡，等待最小冻结控制器修订和原T06/T13基线迁移批准。原收据/日志不改写，观测副本与proposal保存在`evidence/drive/t06-runtime-blocker-20261029T022232Z`。

## 执行器异常清理P1修复已复验 2026-10-29T02:57:36.429156+00:00

ADR027最小4文件修复及原T06/T13迁移经128回归、入口校验、固定round26独立复审、新基线准确A1/完整verify及固定完成提交f21cd25b0539447d29b4cad536aeed5aa0c89762的原独立复巡20261029T024917898675 CLEAR。首次与清理枚举异常的漏登记/漏回收/提前封存路径已修复；旧坏hash及未登记payload历史边界继续保留原样观测，原成功证据按对应基线核对。

## T08在途HTTP竞态直接证据补充 2026-10-29T04:45:16.708535+00:00

固定独立巡检20261029T043409131594 CLEAR同时列出非阻塞缺口：专属HTTP匹配测试已有排队取消、配置变更后的拒绝，在途HTTP取消/改配置竞态直接场景未执行。原通用请求/合成边界证据保持，下一T09检查真实Session/上游取消/旧句柄清理时重点补此相关交互；如实保留验证边界。


## 2026-10-29T08:18:27.245074+00:00 T09独立CLEAR保留的P2与T10相关验证

- 来源：固定da14d4d原巡检20261029T080606981755-T0.9（CLEAR）。已确认HTML分支及正文探测MEDIA_UNSUPPORTED与SPEC9.2要求UPSTREAM_FAILED不符；只转发Range/If-Range遗漏If-None-Match。待验证gzip自动解压后强ETag与实体是否一致，当前fixture无ETag，未作运行确认。
- T10网关清单/响应头范围内补上述相关最小行为及HTTP反例；保留源上下文/源鉴权隔离，If-None-Match实际响应处理，HTML诊断分类和修改后Length/Encoding/ETag一致。原T09成功/失败与CLEAR不回写。

## 未复现：T09网关测试偶发超时 2026-11-04T06:45:13+00:00
T0.13完整verify（evidence/verify/20261104T063504851866.json）中gateway::tests::t09_slow_timeout_truncation_disconnect_and_close_cancel一次失败：fixture wait_for超过5秒，日志无行号。同源单独20次（含16个满载CPU进程）、完整cargo test 3轮、其后完整verify均通过；原因未确认。当时另一项目的容量测试在同机运行，影响未验证。未验证线索：该用例/stall夹具停顿5秒，与quiet的5秒等待上限相同。再次出现时开RUST_BACKTRACE=1取行号。

## CatalogView首页分支已无入口 2026-11-04T08:02:42+00:00
T0.14起原生首页由HomeView渲染，src/views/CatalogView.vue的home分支和源选择器（route为home时）不再可达。该文件不在T0.14范围，留待T0.15修改详情界面时清理。

## 中文界面来源校验提示为英文 2026-11-05T02:46:28+00:00
validateSourceDraft直接返回英文文案，T0.17复巡evidence/reviews/20261105T023036251067-T0.17报告的P2，尚未修复。

## T0.18巡检保留的历史展示P2 2026-11-06T01:56:17+00:00
来源：巡检20261106T015109203950-T0.18（CLEAR）。原生历史条目把集号向上取整且episode_total等于当前集，剧集第1集会显示为“Feature”、2.5集显示为第3集；海报固定为占位图，未用poster_cache_key或当前元数据补齐。数据库保留原始季/集号与video_id。T0.18范围外的展示修正留待T0.24端到端核对时处理。

## T0.35 交付文档与后卡生命周期入口 2026-11-10T06:18:00+00:00

- 独立只读前置审阅确认 README.md 顶端仍记载旧的构建待执行/链接器签名状态；该文件在本卡范围外，交 T0.36 的文档交付范围按实际最终制品更新。当前事实已在 packaging/T23.md 同步，历史事实及原失败保持。
- ADR-058 已要求 T0.36/A2 使用升级后/卸载后 SQLite user_version=2 并排除 metadata_preset_applied 设置比较；当前 t23.py lifecycle 仍保留旧值，按该 ADR 在最终生命周期人工执行前修订并重验。本卡只执行当前安装包检查，不进入 T0.36 安装生命周期。

## 后续集数优先使用已验证线路 2026-11-10T06:39:00+00:00

- 来源：外层在 T0.35 handoff 的 2026-11-10T06:36:25.305034+00:00 转达用户新增需求：某集尝试多条线路找到可用播放源后，同一剧集列表后续集数默认优先使用该可用线路；不可用时沿现有流程尝试其他线路。外层明确当前 T0.35 固定长播验收先继续，新需求后续单独调查实现。当前受验源码、任务和标准保持；后续确认跨集线路身份映射及失效行为后按当时授权实施。

## 数据库 v1 迁移测试与 ADR-064 冲突 2026-11-11
- T0.38 回归时 `tv5_migration_failure_rolls_back_and_old_database_remains_usable` 与 `tv5_migration_v1_keeps_all_history_fields` 失败，错误为 `Unsupported history database version`。原因是工作树中按 ADR-064 删除数据库 v1 迁移的 `history.rs` 未提交改动，这两项测试仍按旧迁移断言。需按 ADR-064 删除或改写这两项测试。

## 新增 Rust 依赖未登记许可证 2026-11-11
- T0.42 新增 `tauri-plugin-dialog 2.8.4`，随之引入 `tauri-plugin-fs`、`rfd` 等 crate；`licenses/dependencies.json` 与 `licenses/THIRD_PARTY_NOTICES.txt` 尚未更新。打包发行前需补齐（T0.44 引入 FFmpeg 时一并处理）。

## 新增 Rust 依赖许可证已补齐 2026-11-11
- T0.44 重新生成 `licenses/dependencies.json` 与 `THIRD_PARTY_NOTICES.txt`（444 项，含 FFmpeg、tauri-plugin-dialog/fs、rfd、ffmpeg-next/sys-next），`release.py --verify-licenses` 通过。上一条「新增 Rust 依赖未登记许可证」已处理。

## release.py 末尾的旧版升级对比检查 2026-11-11
- `packaging/release.py --build` 在 dmg 生成并校验后要求「Real build 1 predecessor」，用于旧的升级生命周期对比。ADR-064 已移除旧版本兼容，这一步应按 ADR-064 删除或改为可选。

## 播放服务第四期（ADR-077，不建卡） 2026-11-11
按 `docs/FOLLOWUP-PLAYBACK-SERVICE.md` 第 11 节，第四期为可选：媒体转发 `/v1/media/{ticket}`（默认关闭，按来源开启，Q7）、热门影片对应关系预取、APP 后端接口（T3_APPYSV2、T3_APPGET，Q8 暂不做）。前三期稳定后再评估。

## 播放服务第四期补充（ADR-078） 2026-11-11

向其他 Stremio 客户端输出 `stream` 移到第四期可选；前三期对外只提供 catalog/meta，App 播放统一走 `/v1/lookups`。第三期接入第三方 Stremio 播放扩展继续保留。本次按个人娱乐应用简化三期验收，长播、容量压力、多平台安装及完整故障矩阵按实际需要再安排。本轮仅修订与复审文档，开发等待用户新指令。

## 播放服务任务编号调整（ADR-079） 2026-11-12

前三期正式卡号为 T1.1–T1.15（分别为 5 张元数据、5 张播放选线、5 张接口适配/联调卡），原移动阶段顺延为 T2.1–T2.6；第四期可选内容继续保留在本 BACKLOG，待前三期稳定后按需要安排。
