# Round 40：T0.13 唯一第六会话授权独立文档复审

结论：ADR-041及对应RESOLVED准确落实真实人类授权。唯一一次5→6、仅tests修正真实结束前提、原12秒共享预算、身份反例和完整复验要求明确。当前证据支持夹具导航过早，生产EOS处理暂无新确认缺陷。未发现阻塞本次授权增量的文档问题。第六会话修复与新源业务复验均为NOT RUN。

## 受审基线

本轮基于round39，仅审查本次授权增量及相关因果证据。

- 唯一项目根目录及全部命令cwd：`/Users/eason/jp/application`
- 固定HEAD：`14c269b0dab7b6f4a7fc4ff34e41019132716901`
- 原摘要函数实际source：`d63da9cc42fc9e831636797d0bb2eea1d102864d6aadec574d5aa72136ef7358`
- 原摘要函数实际protected：`97a7e1c6b4841e756b3f9c7a5749895fb0b41a58c6a86c2eb92aa38bc0f9f10b`
- ADR-041分段SHA256：`1b351c8fe55bdb85ae2dfa9971f49d97127bd1e61fdfadf28bc194c0028d373d`
- 工作树干净。固定提交仅追加DECISIONS 11行、PROGRESS 7行，两文件保留父提交完整字节前缀，前40份ADR分段摘要保持。

| 受审文件 | SHA256 |
|---|---|
| docs/DECISIONS.md | `b3154b4c07a906acedc5671c9c9ee6185e941c83c62b701cb6f9a31227ecb3ad` |
| docs/PROGRESS.md | `793d47c036ed268ae647c71ec42827a2eaedd583c6f17034301d10f965d7b780` |
| docs/TASKS.md | `1ab319bff84d3ee4a9f54544f93dbcf4f473785252ce7723f8a01d240f6ed396` |
| docs/SPEC.md | `a9f03980c41e38f8a2e06f0f33d80b3f0262313b3f7b67d579dc092e8478f5b8` |
| docs/ARCHITECTURE.md | `603685f99ba650e7488f05a26dc5fe81c434b63f1e126de923e0d999a2fcf625` |
| workflow.json | `829f62c83680b409d58e67807922ea9e15d694ebdfa9573250884bb247554bf7` |
| scripts/workflow.py | `8365e061e9bb8600d74b10644956c757af9ec3090ff4176b69ec1e43203bb01e` |
| docs/reviews/doc-review/round-39.md | `24954cc56ba12ca44774e3ee00e26009e30c5aeb607b7b5922b2a57dbbc6c226` |

## 真实授权、语义覆盖与可实施性

沿用既有moc-init、AGENTS及PROMPT-DOC-REVIEW要求，本轮核对round39、原T13义务、SPEC11.4、相关结束/状态契约及ADR041/RESOLVED。

root可信转述的真实用户回复为“仅批准 T0.13 追加一次（5→6）”。此前询问明确说明第五额度耗尽、107份证据封存，测试仅见ended属性便导航，此时可信结束事件未记录、正式状态仍playing并保存未完成；仅增加一次，由原CLI仅修测试结束前提、补正负例并完整复验，真实结束、原时限、全部门和生产EOS语义保持。受审文本准确记录该决定。

新增范围仅tests/PROGRESS：在当前video、epoch/session的真实trusted ended、正式控制器完成及对应HistoryEntry完成提交后，再执行原导航、Play again、从0重播、删除清空及焦点断言。原结束等待12秒为共享预算，明确禁止串接新的12秒扩大期限。

正负例要求覆盖真实当前结束与提交正例，以及错video/旧epoch/session、合成事件、state-only和仅ended属性等拒绝条件；禁止补造completed、进度阈值推定完成或修改生产EOS/历史语义。该约束能拒绝以属性快照或人工状态冒充事件链完成的实现。

原14冻结cases、13项独立oracle及行为正负例要求、单AppState/Api/SettingsQueue/UiProvider/useOsTheme、路由来源和epoch守卫、双语偏好及会话保持继续受验。20矩阵、4DPI、五色首帧、4.5对比度、确认/焦点和真实main成功0/故意失败1探针保持。

新源仍须完整50切集、51份aggregate CPU/RSS及十窗口原趋势门、逐轮帧PCM/Session绑定、单播放器和全部资源回收，以及C9一次3秒查询、一次过期恢复、图片恢复、完整errors、autoplay、unknown、真实EOS、布局、正常stop/dispose及Host finish。旧源结果保留历史，原930秒和其他期限保持。

History继续为会话预览，SQLite后续卡接入；移动仅结构范围。生产源码、范围外Rust、模型/依赖、冻结契约、控制脚本和全局配置保持，本次无需rebaseline。

## 原始因果证据核对

当前指针指向：

`evidence/drive/t13-fifth-validation-20261104T033248Z`

指针SHA256：`9fe7adac913b076c1c6a87d981a1ad4e5144c12361285359e5798f2595763483`。

`history-ended-timing-failure/manifest.json`的SHA256：

`5a4bd9c56fe05cf0e6d1c6cfca3ae171c230e95154edc94760f6af7f1503b814`

107份副本全部存在且符合记录SHA。受限归档`restricted-source/manifest.json`的SHA256为`7bf71f43ccf8e59a554921423225ebe30c1549f001510a3950fd48e36e855ff1`，其10份副本也全部符合SHA。

独立核对原UI失败trace：

- 55条，order完整连续1至55。源码100条上限未触发，当前记录未截断。
- native-ended与delivered-ended均为0。
- order50：实际video.ended=true、currentTime=30；正式epoch120/session仍phase=playing、position=29800。
- order51/52：立即导航，调用并完成保存completed=false。
- order54：列表读回completed=false。
- order55：动作仍为Continue watching。
- 全部save-called中没有completed=true输入。

因此，当前记录没有“完成写入随后被覆盖”的证据。源码中的delivered标记来自事件后的queueMicrotask状态观察，并非正式controller回调标记。ADR准确说明该语义及证据限度。

当前失败确认夹具在正式完成前导航；生产真实ended处理暂无新确认缺陷。共享期限内等待真实事件、正式完成与提交后的修正效果仍须新实测。

第五CLI留存launch/result记录原gpt-6.1-sol/workspace-write argv、session5及正常agent_exit0；本轮据此核对存档，未补造中断前实时CLI创建身份核对。受限自验原始125归档保留cleanup_failed、raw_exit/tests未知和早登记payload事实；后续finished125恢复收据另存，原失败未改写。

外层相关原run为finished，source前后一致：13 oracle、136行为、类型、格式、native check与Clippy通过。UI诊断预检退出1、3项2通过1失败；主UI11项10通过1失败，真实成功0/失败1退出探针及remaining=[]记录保持。

后半重播、删除清空与焦点、五startup、第五源准确A1及完整verify均为NOT RUN；旧T13-all仍为第三源失败。

## 预算与完成门

实际严格首张未完成卡、current及原`next_card()`均为T0.13；T0.1至T0.12完成判定成立。当前s5/f8/generation3、paused=false、hard_sessions字段不存在；保存的`sixth-authorization/original-status.json`一致。第六许可未消费，第七未授权。

固定独立round40 CLEAR原文保存提交后，紧邻严格顺序、所有旧创建身份、两锁实得、clean/source/protected及s5核对，可通过唯一一次原`drive --once --max-sessions 6`落实本次累计额度。实际s6即消费，其他卡default2，失败、segments、rebaseline及全部归档/WIP历史保留。

原模型权限、4096MiB macOS RSS采样、jobs2、线程1及session10800/idle2700/command7200/review1800等期限保持。GUI锁定时等待真实解锁。原CLI负责授权测试修复及真实相关自验，受限时按ADR010稳定WIP退出后交外层原run代验。

新源准确A1成功后直接完整verify、原done、受验提交、新固定独立CLEAR及clean全齐才完成，后卡等待。未发现本次增量造成的新文档路线阻塞。

## 实际检查与未验证项

本轮仅执行文件阅读、Git只读检查、原摘要及控制器只读函数、107份当前与10份受限副本SHA256核对、原UI trace顺序/保存输入重算及相关源码和收据检查。全部命令显式使用唯一项目cwd；Python仅使用`/Users/eason/jp/application/.venv/bin/python -B`。未写文件、状态或Git，未测试、启动CLI或取锁，未输出凭据。实时旧身份、两锁及GUI仍须启动前紧邻核对。

第六会话夹具修复、相关正负例、真实WK完整历史回归、新源准确A1、完整verify、done、受验提交与新固定独立完成巡检均为NOT RUN。文档CLEAR确认本次授权与规划充分，业务完成按后续真实证据判定。

DOC-REVIEW-VERDICT: CLEAR
