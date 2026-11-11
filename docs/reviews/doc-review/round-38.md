# Round 38：T0.13 唯一第四会话授权独立文档复审

结论：ADR-039及对应RESOLVED准确落实真实人类授权。唯一一次3→4、真实播放与UI失败诊断、确认原因后的最小修复、完整复验及原完成门明确。原验收强度保持，现有证据不足的根因如实标注。未发现阻塞本次授权增量的文档问题。第四会话修复与新源业务复验均为NOT RUN。

## 受审基线

本轮基于round37，仅审查本次授权增量和失败义务。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`0ead3e2a526b9c12d08709432a0e9b48469a133e`
- 原摘要函数实际source：`a2120f9d02c862fa5b02114c8cb2596e1d56d435d4e4a88241738004325b6c4b`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-039分段SHA256：`6b8183fefb77f5018d28a45a1371973dc79e250b089598bc2e01007222e3546d`
- 工作树干净。固定提交仅追加DECISIONS 11行、PROGRESS 7行，两文件保留父提交完整字节前缀，前38份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `09ff603031e640cd1043d1363013a477d910a6fabd340f90ec191a22b9b79f7d` |
| docs/PROGRESS.md | `1f7d12883335218d35e5cb9191c26aac5c52ec43831cad30512417f74b799779` |
| docs/TASKS.md | `1ab319bff84d3ee4a9f54544f93dbcf4f473785252ce7723f8a01d240f6ed396` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-37.md | `9d6cc97aede8c04a7e8f222ccc2275f5554eea92e0a33a51f72a9e78072bbac4` |

## 真实授权与覆盖结论

沿用moc-init、AGENTS及PROMPT-DOC-REVIEW，本轮读取原T13完整卡、SPEC8.1/10.5/11.1/11.3、C9/C10/C11、round37、ADR039/RESOLVED及相关失败证据。

root可信转述的真实用户回复为“仅批准 T0.13 追加一次（3→4）”。此前询问明确告知第三额度耗尽、50切集/音画/RSS门通过、过期恢复及三项UI检查失败、程序写报告后仍超时、部分根因待诊断且90份证据封存；仅追加一次，原模型权限定位最小修复并完整复验，原范围、全部门及资源时限保持。受审文本准确记录该决定。

本次工作直接对应T13原恢复、返回筛选、外观可读性、历史确认/焦点及真实UI验收义务。ADR要求补正式route/context/session/resolution/epoch、资源身份与次数、video/controller状态、实际role、selector、对比度样本和异常信息，先区分生产接线与夹具前提，再修确认原因。原生程序正常退出要求实际退出证据及不吞错的负例，原930秒期限保持。

原14冻结cases、单AppState/Api/SettingsQueue/UiProvider/useOsTheme、交叉身份、来源分离、host/guest失效、旧epoch与取消、首页合并和图片裁剪、默认英文/中文、系统明暗与五色首帧、设置失败确认值、路由/筛选/选集和播放保持均明确保留。20矩阵、4DPI、390px、五色首帧及独立非零正负例保持；对比度继续采用既有4.5判定，要求保存真实测量诊断。

C9一次3秒只读查询、身份校验、一次过期恢复及第二次拒绝/手动retry、图片真实一次DTO恢复、完整401/502/codec/track与完成后重复诊断保持。原50切集、51份aggregate CPU/RSS、十窗口原趋势门、每轮帧PCM、Session/URL、单播放器和清理门保持。禁止抽样、warmup、强制GC、同源筛选或改阈值测法获得通过。

原范围仍为src、lib.rs/main.rs、tests和PROGRESS；History保持会话预览，SQLite后续接入，移动仅公共结构范围。真实WK、生产与prototype回归、CSP/Origin及单窗口安全门保持，冻结与范围外Rust、依赖、控制脚本和全局配置保持不变，无需rebaseline。

## 当前失败证据与不确定性

当前指针指向：

`evidence/drive/t13-third-validation-20261103T084301Z`

指针SHA256：`befa32191a2d8fe26d5c67065d3307d304803ea79ee09ea0553edc7296e03237`。

`native-acceptance-failure/manifest.json`的SHA256为：

`5654a74d12942cce79ec8f62c0434b07154316e670c228833bc78fbce0def49c`

90份实际副本均存在且符合记录SHA。缺1份可选周期`samples-1.jsonl`准确列明；报告保留51份实际aggregate样本及逐命令结果。

准确A1 `20261103T084408161514-A1`为finished、exit1、s3/generation3，source前后一致。报告106必需、106实跑、104通过、2失败；JUnit120项，107通过、13失败、0跳过。两项失败为过期会话恢复及native UI超时。原第三会话相关oracle、行为、类型、格式、native check/Clippy收据均退出0，12 oracle及134行为结果保留历史。

保存数据确认50条切集、51份样本、nativeHLS与controls结果存在。十窗口RSS中位数重算为`298139648、273301504、265994240、263749632、272007168、272269312、266518528、274612224、264077312、265109504` bytes，满足原趋势门。保存媒体证据的原run退出0，仅校验已保存结果，准确A1失败保持。

过期恢复报`Expired session did not recover`；现有generic video诊断未提供正式App/controller完整失败状态，具体生产或夹具根因仍待定位。

主UI报告11项，8通过、3失败，失败仅保存stack：

- 返回筛选停在browser.js第98行。
- 对比度停在第125行调用及textContrast第66行，实际失败元素与测量值缺失。
- 历史删除确认停在第234行等待alertdialog，实际role未保存。

已只读查看删除确认截图：模态存在，Cancel按钮有可见焦点；截图无法确认DOM role及完整焦点行为。ADR准确要求补该诊断，未预先认定生产缺少确认框。

原native helper在写UI报告后仍存活至约930.068秒；原命令exit为空、finally remaining=[]。记录保留该退出异常，未改写为正常退出。

后续图片、完整errors、autoplay、unknown、EOS、layout、正常stop/dispose、Host finish、五色startup和完整verify为NOT RUN。现有失败检查及程序退出问题仍待第四会话修复与新实测。

## 预算、路线与完成门

实际严格首张未完成卡、current及原`next_card()`均为T0.13；T0.1至T0.12完成判定成立。当前s3/f5/generation3、paused=false、hard_sessions字段不存在；保存的`fourth-authorization/original-status.json`一致。第四许可未消费，第五未授权。

原控制器累计上限与启动递增语义，配合固定独立round38 CLEAR保存提交后，紧邻严格顺序、旧创建身份退出、两锁实得、clean/source/protected及s3核对，可落实唯一一次`drive --once --max-sessions 4`。实际s4即消费，其他卡default2，历史不重置。

原gpt-6.1-sol/workspace-write开发和read-only巡检、4096MiB macOS RSS采样、jobs2、线程1、session10800、idle2700、command7200、review1800及全部安全时限保持。GUI锁定时等待真实解锁。

原CLI实施并读回相关真实自验；受限时按ADR010稳定WIP退出后交外层原run代验。新源准确A1成功后直接完整verify、原done、受验提交、新固定独立CLEAR及clean全部成立才完成，后卡等待。未发现本次授权增量造成的新文档路线阻塞。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、保存报告/收据与失败源码核对、RSS重算、90份副本SHA256校验及一张确认截图查看。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未测试、启动CLI或获取锁，未输出凭据。实时旧身份、两锁和GUI状态仍须启动前紧邻核对。

第四会话诊断、原因确认、最小修复与回归、新源完整准确A1、完整verify、done、受验提交和新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权和规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
