# T1 修订后独立完整复审 — round-62

日期：2026-11-12  
结论：D003–D007 已解决；新增 D008 一项验收遗漏，状态 OPEN。本轮结论为 BLOCKING。

## 受审基线

- HEAD：`d2dc781b27f2f52289675f2c8f21ec661d9a14e3`
- TASKS SHA256：`718dc6a232f80b35691958cdc2c7daf2946c6f4932733bd1da27437907b9e047`
- 方案第 4 版 SHA256：`43e65d955c6da27ab63b25e27fef2fcfdd9f25fbebc2f7c024011ce68254b069`
- ARCHITECTURE SHA256：`f95e1f6cb076664d0db8aec6c2b7382645d3219f27e83c2dea1449ea8af71199`
- DECISIONS SHA256：`2318d716460b94823fc3e5e35e6bb3cb5d17c8984c4285045d77aaf5feb358f7`
- AGENTS SHA256：`70d95a5fb66de7a8fd525dc736901ea835f3617c8598f080b744d367dff98c61`
- ADR-080 摘要：`0fac3bd17d9141865a363e14865cc26f3ac0103eeb07148488f0c3cb23654bbe`

复审期间观察到其他规划分支临时改动共享目录。最终于 UTC 04:08:20 重新采样，受审文件及 protected 已恢复并精确匹配本轮委派的固定版本。本报告绑定以上版本。

DOC-REVIEW-BASELINE: {"protected":"73380819fe8007207eca1d1e0979b56607968c9ff83f48ecbba182e3245fc808","adr":{"ADR-001":"31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407","ADR-002":"d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368","ADR-003":"26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c","ADR-004":"a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae","ADR-005":"df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d","ADR-006":"335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1","ADR-007":"028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227","ADR-008":"9c1b6bc1afa7ae46d9c714f2da6ff25775bb4f84fa2a894666823fabbc9f00cc","ADR-009":"2af85af3e9a0d33cd2fe5088a27bc3075ed73986e2e6da5b904129753f6e62c6","ADR-010":"2a8794076a5170988b290a41e762b733d1fca797f403b3ec2a497e32cbf4f45b","ADR-011":"48be2fa1e537f273c097ea8a1740ed0d93e8ea0dc3fa22abb8ea87dee831297b","ADR-012":"6b5c36ac582bfaea4cf97d42f35a2ea515abc903e9b835631469fca4200ed5c0","ADR-013":"d2c0c16b73cb276623fa0609ef43d8004ffbe78f4fe876c9211ccf7050294abf","ADR-014":"ef2a5363e382e5ddd1f085a1f7ae8671e9e420ed2eca8f565d6a5ef58ec78017","ADR-015":"22efa652b4d7491e68730f18d581764a679f8ed76baed2b673cd588ee93e7d48","ADR-016":"7bafb564f37f4c8f97dc2dd5984b1d566f621a0b96456e4c93fbf55f504862e8","ADR-017":"af981e1d604c68cb3971f88d8f8a404ee7e96c54a252946739f617b1a1776641","ADR-018":"3afac14679cd0259aa796032d3dc17c98a9915bd1eaadb2f80f2c19cd2ab22c4","ADR-019":"76f109d971d15c3c1b3f5377468a2e5a746c38f0caf3357442fd5d10745125bc","ADR-020":"4deb9a3d81c7033670e543644299bd456e0ec2eabfeeb25d96cb3e05b87fc9b3","ADR-021":"162b25bbf3de89b4a4c2fb1a6ed5ba2791fad963300807d7781f74e5e13025a9","ADR-022":"f5d9c3f48b001f688a18044d80dccbb75c98cf6a71b8627395e9acc2169b5c57","ADR-023":"f18853d93cb1d48819e9e84d9c3185f94d80bac46b42e99f9f6244692b277478","ADR-024":"038231a74a95bd84aafcedbe60ef46676b9f1fc63329bea5aff62a515c5a49cd","ADR-025":"4daf92c6b76cb0f449eed044f1ab2a6878df0ffc70a8653967a169780f1eb572","ADR-026":"a215bc09b327d0b295185c916805e5a2448f2b6b208591c66fab2c8bdad6052c","ADR-027":"428a4bc585fe0107c643716f76e6857e19534886ff647556d02477fe7495833d","ADR-028":"46c40a9b9cfbc2aed67690189d5f787e4a860d8c1c73a1649c07a5eecd41ee27","ADR-029":"8a8b072d7f01fb2418d7b3616d5f1697cdc50ebf6437d2bafc24ed272dbc2b5b","ADR-030":"cf95dc1bc8f9b096d1c4549bd0c8c5b6cbc58978973bd005d1e7b93d5b8d5549","ADR-031":"44ffce2c67b234c75cd80a8dce2846d57a8c71a70fcbb0c3e965f35922beff98","ADR-032":"c7417df2199f93cde98bd89d2bd3c61ec1ebb8473c35f3662cce2e5ecda1580e","ADR-033":"b7ac3a6796442e301c136ccb6421ca79c86ec43e261bc676504011cfe37fdde0","ADR-034":"37b10ad4e7301e28c2f69ad98212780deb59cdee2f991d205edb215fa7265d95","ADR-035":"6d5674a809870c75775f8b1f52d91cf098e1de30b3582c622f2547269a852946","ADR-036":"87bb8edf873c645c7e7228f365e78d6f0ea9ad1be7ca7a4679cbda73fb50087c","ADR-037":"268ce9b4aceff1c9fff59c8d8d7083212ef3dc9618224ecc55f8ffb9a386ef33","ADR-038":"e4a2efbfd5e847714bd91e065e18ed4346840f47228afa49f54d64b6801e9528","ADR-039":"6b8183fefb77f5018d28a45a1371973dc79e250b089598bc2e01007222e3546d","ADR-040":"2ab090ca8a1dc75d76515a25327906211bb4aede86574904728215312e14b171","ADR-041":"1b351c8fe55bdb85ae2dfa9971f49d97127bd1e61fdfadf28bc194c0028d373d","ADR-042":"f65e8d98ede2c94d2b1f1af780a87d11dfe674ad39a32966d0e23ed563d20c3e","ADR-043":"85de7df8d21029d6ef84b17b29db997fe318e95dba2187207c15cc3340b706f9","ADR-044":"f6e19d72435fb278213b117f08beae149c52695865a1c4bd45c36ebcbee44a44","ADR-045":"bb77b617437541b76a491b34ac823795879fba9a4a2912596f6919aaba7fbfa5","ADR-046":"42058f815bb3a0cda323014c430ac257157d9b8edd35c0cbf90956781aa2b446","ADR-047":"cab240e7dc9c1ab657fb02aaaf820d45cf69577a145679548624a1473512c012","ADR-048":"bdedb085c22082216d15421f21e2ea10e3afe61cce97080e9c9f180408b858c0","ADR-049":"af60be8e05e3968e605ccad0552d56ec96540b66e1529fc1337b94b43d54e601","ADR-050":"4f747a4e41435d2317e9ccbbfbcd0e4d12ff98a35c01b73c40d9c3b43829623a","ADR-051":"d3223c550c13f153b1a3ccd366898f693f12bcebf786349bc1352248cfeb5f06","ADR-052":"34dc063470d005834931ef0b845898560bdc78aa156e4ade27c7ee38579da852","ADR-053":"e462aa8d9adcc26125e5617a7ba7c0e9ceadb789fe4f3f403d5e79d260cc21e3","ADR-054":"68d96ac9338baccd6cfd30f9c6fcc5508ba3ab784431ce92546d992d5338bd3a","ADR-055":"4a141790900f750657098ef109ac0937efa8fb3263d5e5495dedf82ccd087533","ADR-056":"0a17f399c0ba0593cbe22684b337ad7af78ca5bcae4aaa90f02d00663c47f5db","ADR-057":"ae40de6cbe055b68d91f45fd3bf97740849a75da729dd54b01abb38f50921ef7","ADR-058":"0b5d21df283072c98822e489c7e530a1250ccc25f7eb396999be40d1d3fba5c3","ADR-059":"9b1a90a73b659e0f4f65ce5b90f74c57ed63b9c0743fa5f4f84c8c555b374914","ADR-060":"d17c70909fcf4a8a389dbb7ee5277076f5482ed3a691895ebede8213f7c700a5","ADR-061":"61e75602eae76ca965dbdf8637d55c9ade8b0f0767b54c604f1f8d01157bed9c","ADR-062":"836f8aca1511dae4e23d3abebc51e792522d32285059d9fb28dd044adf5e573a","ADR-063":"92e51703ac49ef0fd0c93f7ba57f4474bccec4815135b1879c1599d2d934f87c","ADR-064":"f07177c64a2a49b93f704a055ad545e65e7302c7f3e2710a6328b0fcf5ab5fb9","ADR-065":"8e3467f9ae949f71b80e3a5456fb13b4b2ef0212b758725cbddefa168d7795f5","ADR-066":"0c4c68a8a309f0869a188547466ac0f01ea769d51428f7201a980b1f5a865499","ADR-067":"4566f8ed549081c381927d6bb8ebdb0ac937c6aba60d09ea79bdeef53475acb4","ADR-068":"227d78b31d7b9ee14b5cf8c38a44b4afd68519cc20ab0e0510ad720182b10d0a","ADR-069":"60f681021f437432a1d998454f30905910eba235cc972dbab9c16fc1255b162f","ADR-070":"720f9ddb311c4ded3d7f1e247e69e4e59f5e69d4b307f3284cb58c0073057acf","ADR-071":"32be782c8bbe185758cf2516cfc4389ed078d0253f8c7133b0eaf950d3ff2b68","ADR-072":"32b2230cb9ef974553f1eeb77c251f5e08943e017461636d66bfdabcb7a8c217","ADR-073":"be787bdc6d4630492095b8642e90a2abf450ec16079e3d2aacd268d67cc590be","ADR-074":"24053a5ad104133f2c9e8ce3d8a133066ceb6fee367d3c6121c1d4ddf08610c2","ADR-075":"49d094186fe6143258d14adf32b014325767925cc4f5f439f368a1aac9168254","ADR-076":"0e1e83bc6754c08b4074d649875b9b1a58c50d5c5e9b098ecbf14de3a4129c0c","ADR-077":"77ec10579866c0d8ae12888cdbf6a6a33e3d43133c67bbe442a3f8a3ce298068","ADR-078":"ac35750afc3ba7a4b28ba185458d8ee5dd360ed9e4af3dc1941cdfafc6920f8d","ADR-079":"d97712523f23bee44aa48aceb17fb2e5822e734c09ba11057dcc4cb8e52f8e2d","ADR-080":"0fac3bd17d9141865a363e14865cc26f3ac0103eeb07148488f0c3cb23654bbe"}}

## 完整覆盖与跨卡复核

已阅读方案第 4 版全文、全部 15 张当前 T1 卡、架构生效补充、ADR-080、原始 D003–D007，以及相关现有宿主和前端代码。

- T1.1–T1.5 仍覆盖服务骨架、鉴权、TMDB、缓存首页、自定义 Stremio、App 接入及当前数据保留。多 manifest 的身份、分组、顺序和部分失败处理已明确；App 搜索验收有一项省略，见 D008。
- T1.6–T1.10 从来源规范模型贯通匹配、映射、lookup、短期线路、原生测速和实际播放。新增上报命令、可信播放 context、新历史写回及离线本地/直播均有对应消费者和直接验收。
- T1.11–T1.15 保留各适配分支及最终汇合。适配器输出逻辑定位信息，公共刷新层处理唯一匹配、重排和 410，解析器保持父线路归属。
- 查找/候选与已签发线路分别保管，取消查找后候选失效；已签发线路在自身有效期内供刷新和上报。选定 Resolution 独立持有 context，再交给 Session，打开失败和关闭沿既有释放路径收回。
- 已明确验证 `play_line` 后、`open_playback` 前取消父轮次的交接时序；活动 Session 的进度保存独立于远端短期句柄，上报过期仅影响统计。
- DAG、T1.11–T1.13 分支、T1.14/T1.15 汇合及 T2 编号保持一致。开发授权门关闭。普通卡与三期 App 收口的验收分工保持，未引入长播、压力或平台矩阵要求。

除 D008 外，本轮未发现新的实质缺口。

## 旧问题状态

| ID | 修订与独立核对结果 | 状态 |
|---|---|---|
| D001 | 首卡仍包含服务接口契约的修改范围和交付要求。 | FIXED |
| D002 | 原生命令注册、权限生成、生成类型和契约检查范围继续保留。 | FIXED |
| D003 | 方案 §8.3 新增受限 `report_line_playback`；宿主从 Session 绑定取得线路和延迟，按会话/播放轮次/结果去重。T1.9 验证绑定、去重和失败处理，T1.10 验证实际推进/失败到服务收到报告，并检查 ready/seek/登记阶段及迟到事件。 | FIXED |
| D004 | `play_line` 核对来源/版本及目标归属，建立完整受信 Resolution context，再向 Session 传递独立绑定。T1.9 检查正确历史保存与伪造身份拒绝；T1.10 检查新记录写入、SQLite 回读及 App 重启后新轮次恢复。 | FIXED |
| D005 | T1.10/A2 在同一次服务不可达场景实际短播本地文件与受控直播，观察时间推进并核对各自原生路径。 | FIXED |
| D006 | 方案 §4.4、§9 和 T1.4/T1.5 明确逐 manifest 持久映射、原配置分组与顺序、整组成功回读后激活、部分失败保留完整原组；样本覆盖同配置两扩展、相同 raw ID、历史/收藏及重启。 | FIXED |
| D007 | 方案 §5.9 明确逻辑 locator 与短期 line ID 的关系，唯一匹配才刷新，消失/歧义返回 410。T1.8、T1.11、T1.13 验证重排及失效，T1.15 验证 App 重查和进度恢复；缓存与统计保持同一逻辑键。 | FIXED |

本轮解决既有问题 5 项，新增已确认问题 1 项，未决 1 项。

## D008 — 一期 App 搜索操作在验收改写中省略

- 类别：已确认缺口。
- 严重度：P2。
- 状态：OPEN。
- 需求：REQ-SERVICE-META；方案 §2.2、§4.4；原 T1.5/A2 的一期 App 搜索流程。
- 位置：[TASKS.md](/Users/eason/jp/application/docs/TASKS.md) 的 T1.5/A2 `description`，以及 T1.5/A1。
- 证据：原 T1.5/A2 明确要求从服务浏览 TMDB 和自定义 Stremio 的“目录/搜索/详情”。当前改写为“目录/详情”，A1 也未明确实际 App 搜索。T1.4 继续验证服务端搜索。
- 因果与反例：服务端搜索可以通过，而 App 的 `search_catalog` 仍请求旧端点、错误代理路径或返回错误扩展身份。当前目录、详情、多 manifest 迁入和已有记录场景均可能通过，搜索页仍无法正确找片。
- 影响：第一期更换元数据传输入口后，核心找片流程缺少 App 接线验收。
- 最小修订：在原 T1.5/A2 恢复“目录/搜索/详情”；复用同组两个扩展的样本，实际搜索并核对结果及后续详情回到正确扩展。继续使用现有 suite 和短时操作。

## 实际检查与未验证项

本轮实际执行：

- 通过项目 `.venv/bin/python -B`、argv 和 `shell=False` 调用 `scripts/task.py check`：退出码 0，66 张卡。
- 使用现有 `workflow.cards()` 对比当前定义与 HEAD：15 张 T1、18 项验收；原 argv、报告路径、超时及 kind 保持，旧 cases 均为当前 cases 子集。
- 51 张 T0/T2 卡的定义摘要与 HEAD 一致。
- 18 个 suite 和报告路径唯一；T1.1、T2.1 授权门均关闭。
- `workflow.budget_problems()` 对 T1 未返回预算问题。
- 使用现有 `protected_hash()`、`adr_digests()` 取得真实完整基线，最终值精确匹配委派版本。
- 阅读并核对现有 `open_playback`、`resolution_contexts`、`session_bindings`、`save_history`、前端播放事件、Stremio 身份及多 manifest 契约、IPC 注册名单。
- `git diff --check`：退出码 0。
- `node_modules`、根 target 和 src-tauri/target 均保持缺失，未恢复构建缓存。

业务实现、构建、服务、实际第三方接口与 App 播放验收均为 **NOT RUN**。本审阅者保持只读，未修改计划、代码或任务状态。

本轮采样 UTC `04:00:56–04:08:20`，约 444 秒，截至最终采样；处于两轮共享 15 分钟预算内。实际模型标识、调用统计、token usage 与费用：**NOT AVAILABLE**。

DOC-REVIEW-VERDICT: BLOCKING
