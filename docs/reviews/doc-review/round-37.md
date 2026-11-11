# Round 37：T0.13 唯一第三会话授权独立文档复审

结论：ADR-038及对应RESOLVED准确落实真实人类授权。唯一一次2→3、容量中断后的最小补齐、独立oracle与UI验收分支修正、原完整验收强度和完成门明确。未发现阻塞本次授权增量的文档问题。第三会话修复、新源准确A1、真实WK及完整verify均为NOT RUN。

## 受审基线

本轮基于round36，仅审查新增授权及T13当前状态闭环。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`b1f0616fa7d9af19212613bf68d35a76bbb1a133`
- 原摘要函数实际source：`ca41e6a49dc3edb08e1a77421fa41cdf64ed596645b1070b38ba19611b0a94a9`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-038分段SHA256：`e4a2efbfd5e847714bd91e065e18ed4346840f47228afa49f54d64b6801e9528`
- 工作树干净。固定提交仅追加DECISIONS 10行、PROGRESS 8行，两文件保留父提交完整字节前缀，前37份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| AGENTS.md | `d5e674d12777f420284574504ab94fed4616ca805556adfc743edf39178044e3` |
| docs/DECISIONS.md | `39a45004fa957ebf4c50bb8f1214557e821c55d8ad6d068b77a0e7337c7134ab` |
| docs/PROGRESS.md | `24c1ddbba0ccf3ffd9db3daaf28e2122476c079d60dd7dd2d9291d5a0bb0ce4f` |
| docs/TASKS.md | `1ab319bff84d3ee4a9f54544f93dbcf4f473785252ce7723f8a01d240f6ed396` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-36.md | `694459bb396aa6ae792a8ee8d075aa9d00601b0660a03cb506c70c790be40279` |

## 真实授权与语义覆盖

已读取项目AGENTS、ADR038及RESOLVED、T13完整原卡、相关SPEC8.1/11.1/11.2/11.3、C10/C11及round36，沿用原moc-init和PROMPT-DOC-REVIEW要求。

root可信转述的真实用户回复为“批准 T0.13 追加一次（2→3）”。询问明确说明模型容量中断、134项前端行为及native check/Clippy通过、仍有四处类型错误、Rust格式、缺独立oracle和UI验收分支错误；仅增加一次，原模型、权限、范围、要求及资源时限保持。ADR038与RESOLVED准确记录该授权。

新增工作限定为App.vue两处genre索引、一处Close词典键、CatalogView模板app.value类型、lib.rs格式、独立`test_t13.py`及validate_ui分支修正，并核对中断后的原实现完整性。现有证据能定位这些问题，均在原T13范围内。

原14项冻结cases及完整卡义务保持：单AppState、唯一api入口与Settings写队列、Route/PlaybackContext/Resolution/Session交叉绑定、元数据与播放来源分离、host/guest失效范围、旧epoch/取消/返回快切、首页全量/单栏/分页与图片裁剪、唯一UiProvider/useOsTheme、默认英文与中文、五色系×明暗×双语、首帧与运行中系统切换、兼容旧ui_theme、lang/title/a11y、设置失败反馈及播放/路由/筛选保持。

主UI交互、20组合矩阵、390px、4项DPI与五色首帧保持非零真实观测；独立oracle至少4项。ADR要求主报告和首帧报告正例，以及缺失/错误矩阵、历史和首帧等反例，能够拒绝仅改分支让不完整结果提前返回的实现。当前`tests/appearance.test.ts:74`已有contrast≥4.5判定，ADR沿用既有外观判定及SPEC可读要求，未新增产品硬门槛。

History保持会话预览、刷新重置、按片合并/时间分组及续播、重播、删除、清空确认；SQLite由后续卡接入。移动本卡保持公共入口与cfg结构范围。原T12播放器、C9诊断保留、真实WK安全回归及其他条件阶段范围保持。

## 当前失败与中断证据

指针指向：

`evidence/drive/t13-second-validation-20261103T080023Z`

指针SHA256为`312024b83c45f38da8749d6f4338b68b373fa731f8128a59ba0521a0c9f7cc75`。

`first-validation-failure/manifest.json`的SHA256为`7a21cb39566d318f7f6fb083e06b279abea6f68917e098999b5f8267b89c5eb8`，29份副本全部存在且符合记录SHA。

`interrupted-source/manifest.json`的SHA256为`0a26b5104aee1ef3829390a515cc666d9ac486eb1844211ee8cf2c83a77df3be`，其5份副本也全部符合SHA。原早登记payload收据保留`cleanup_failed`、exit125、raw_exit缺失及sysctl权限清理说明；当前归档同时保留后续恢复后的finished收据，未覆盖原未封存事实。

原session2 stderr实际包含两次`Selected model is at capacity`。驱动exit1，无正常最终交接；ADR准确记录容量中断，未将其描述为正常交接。

当前原始收据与日志确认：

- 134项前端行为通过，native check与Clippy退出0。
- 前端类型检查退出2：两处genre字符串索引、一处Close词典键、一处CatalogView app.value。
- Rust格式检查退出1，差异位于lib.rs。
- `tests/acceptance/test_t13.py`实际缺失；A1要求至少4项独立测试。
- 合成主UI正例检查退出1，`validate_ui`抛出`KeyError: 'first'`。

分支因果链明确：主套件包含`history-refresh-reset-and-preferences`，当前`any(...)`条件据此进入首帧专用分支，随后读取普通主用例缺失的`actual.first`。该反例验证的是验收函数输入分类，真实WK尚未运行。

准确A1、真实WK及完整verify均为NOT RUN；保存证据注明当时GUI locked及编译/验收前提未齐。

## 预算、路线与完成门

实际严格首张未完成卡、current及原`next_card()`均为T0.13；T0.1至T0.12完成判定成立。当前s2/f4/generation3、paused=false，hard_sessions字段不存在；保存的`third-authorization/original-status.json`一致。第三许可未消费，第四未授权。

原控制器累计上限及启动递增语义，配合唯一一次`drive --once --max-sessions 3`与紧邻顺序、旧创建身份、两锁、clean/source/protected和s2核对，可落实本次单卡授权。其他卡default2，原失败、segments、rebaseline、WIP及隔离历史保留。当前未消费许可与历史许可分开记录。

原gpt-6.1-sol/workspace-write及read-only巡检、4096MiB macOS RSS采样、jobs2、线程1和全部期限保持。A1定义3600秒加完整verify定义5040秒，共8640秒，原单会话10800秒保持。GUI锁定时等待真实解锁，先处理编译及验收前提。

原CLI实施及真实相关自验；受限时沿ADR010稳定WIP退出后由外层原run代验和Git收尾。新源准确A1成功后直接完整verify、原done、受验提交、新固定独立CLEAR及clean齐备才完成，后卡等待。冻结定义、模型契约、控制脚本及全局配置未改，protected保持，无需rebaseline。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、保存日志/收据核对，以及29份当前副本和5份中断副本SHA256校验。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未运行测试、开发CLI或获取锁，未输出凭据。实时身份、锁及GUI状态仍须启动前紧邻核对。

第三会话补齐、独立oracle、UI分支修复回归、真实WK完整矩阵、新源准确A1、完整verify、done、受验提交与新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权与规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
