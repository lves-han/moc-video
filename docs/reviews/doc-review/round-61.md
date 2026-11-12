# T1 系列再次独立只读审查 — round-61

日期：2026-11-12  
结论：新增 5 项已确认的规划与验收缺口，状态均为 OPEN；本轮结论为 BLOCKING。

本轮先独立阅读方案、全部 15 张 T1 卡和相关代码，再阅读 round-59/60，并对作者提供的候选问题逐项核对。审查沿用个人娱乐应用规模，建议均可并入现有卡片和代表性场景。

## 受审基线

- 项目：`/Users/eason/jp/application`
- HEAD：`d2dc781b27f2f52289675f2c8f21ec661d9a14e3`
- TASKS SHA256：`83fc35c625a98c5f57719ce29916fe79e3a9b3335386d4924e17b6ab0f7f89ae`
- 方案 SHA256：`c51ec4c5019fb1f20512a7a00a3c753bc0f93dc41fdc792a8ee6244c0001388a`
- ARCHITECTURE SHA256：`2bb6b95e666cd04fb2af898f1d4d0dbcaefb0bc71412f7beef7b843511ac35dc`
- DECISIONS SHA256：`4a778f8fc4ce46ee4f1de21b9855ce6a8a8d960ae7395e9e2b4e044b92e104d0`
- AGENTS SHA256：`70d95a5fb66de7a8fd525dc736901ea835f3617c8598f080b744d367dff98c61`
- workflow.json SHA256：`75db18b9afa98c97603cfab563bf3628a5917780e64f9ec824d583cb524be1b6`

受审计划与本轮开始时一致。工作区仅有外层追加的 `docs/PROGRESS.md` 记录。

DOC-REVIEW-BASELINE: {"protected":"43fdd084706396b2662856a0d56852d23cccbd6445b4e318d970b2c851f9f08f","adr":{"ADR-001":"31756257a1974108f9c1c8426c9bcb8f80128e2b867e9703f4b766b2e1754407","ADR-002":"d67dcfe2ddb2fd858acd1b1f4cb17440fa47dc15b4948811b1018d062b52e368","ADR-003":"26e4e12c4ae38b3ddcfbea18c04a1602799586a08610125f66567ae8dd516c5c","ADR-004":"a08db017049e19a1481fa314938b5eb88dfcb5598287af24e70bd93058c6eaae","ADR-005":"df0b0d68cd7a4595bd621ce153ba914da82496311c112089fd5b843d8685606d","ADR-006":"335827cabfced06c4cc2e2fc99978940e01818abbd7af6aedd12a66161b883c1","ADR-007":"028d7df4ecd18afa281cb45703aaf4e26857f114163be10b0c2d8860d718b227","ADR-008":"9c1b6bc1afa7ae46d9c714f2da6ff25775bb4f84fa2a894666823fabbc9f00cc","ADR-009":"2af85af3e9a0d33cd2fe5088a27bc3075ed73986e2e6da5b904129753f6e62c6","ADR-010":"2a8794076a5170988b290a41e762b733d1fca797f403b3ec2a497e32cbf4f45b","ADR-011":"48be2fa1e537f273c097ea8a1740ed0d93e8ea0dc3fa22abb8ea87dee831297b","ADR-012":"6b5c36ac582bfaea4cf97d42f35a2ea515abc903e9b835631469fca4200ed5c0","ADR-013":"d2c0c16b73cb276623fa0609ef43d8004ffbe78f4fe876c9211ccf7050294abf","ADR-014":"ef2a5363e382e5ddd1f085a1f7ae8671e9e420ed2eca8f565d6a5ef58ec78017","ADR-015":"22efa652b4d7491e68730f18d581764a679f8ed76baed2b673cd588ee93e7d48","ADR-016":"7bafb564f37f4c8f97dc2dd5984b1d566f621a0b96456e4c93fbf55f504862e8","ADR-017":"af981e1d604c68cb3971f88d8f8a404ee7e96c54a252946739f617b1a1776641","ADR-018":"3afac14679cd0259aa796032d3dc17c98a9915bd1eaadb2f80f2c19cd2ab22c4","ADR-019":"76f109d971d15c3c1b3f5377468a2e5a746c38f0caf3357442fd5d10745125bc","ADR-020":"4deb9a3d81c7033670e543644299bd456e0ec2eabfeeb25d96cb3e05b87fc9b3","ADR-021":"162b25bbf3de89b4a4c2fb1a6ed5ba2791fad963300807d7781f74e5e13025a9","ADR-022":"f5d9c3f48b001f688a18044d80dccbb75c98cf6a71b8627395e9acc2169b5c57","ADR-023":"f18853d93cb1d48819e9e84d9c3185f94d80bac46b42e99f9f6244692b277478","ADR-024":"038231a74a95bd84aafcedbe60ef46676b9f1fc63329bea5aff62a515c5a49cd","ADR-025":"4daf92c6b76cb0f449eed044f1ab2a6878df0ffc70a8653967a169780f1eb572","ADR-026":"a215bc09b327d0b295185c916805e5a2448f2b6b208591c66fab2c8bdad6052c","ADR-027":"428a4bc585fe0107c643716f76e6857e19534886ff647556d02477fe7495833d","ADR-028":"46c40a9b9cfbc2aed67690189d5f787e4a860d8c1c73a1649c07a5eecd41ee27","ADR-029":"8a8b072d7f01fb2418d7b3616d5f1697cdc50ebf6437d2bafc24ed272dbc2b5b","ADR-030":"cf95dc1bc8f9b096d1c4549bd0c8c5b6cbc58978973bd005d1e7b93d5b8d5549","ADR-031":"44ffce2c67b234c75cd80a8dce2846d57a8c71a70fcbb0c3e965f35922beff98","ADR-032":"c7417df2199f93cde98bd89d2bd3c61ec1ebb8473c35f3662cce2e5ecda1580e","ADR-033":"b7ac3a6796442e301c136ccb6421ca79c86ec43e261bc676504011cfe37fdde0","ADR-034":"37b10ad4e7301e28c2f69ad98212780deb59cdee2f991d205edb215fa7265d95","ADR-035":"6d5674a809870c75775f8b1f52d91cf098e1de30b3582c622f2547269a852946","ADR-036":"87bb8edf873c645c7e7228f365e78d6f0ea9ad1be7ca7a4679cbda73fb50087c","ADR-037":"268ce9b4aceff1c9fff59c8d8d7083212ef3dc9618224ecc55f8ffb9a386ef33","ADR-038":"e4a2efbfd5e847714bd91e065e18ed4346840f47228afa49f54d64b6801e9528","ADR-039":"6b8183fefb77f5018d28a45a1371973dc79e250b089598bc2e01007222e3546d","ADR-040":"2ab090ca8a1dc75d76515a25327906211bb4aede86574904728215312e14b171","ADR-041":"1b351c8fe55bdb85ae2dfa9971f49d97127bd1e61fdfadf28bc194c0028d373d","ADR-042":"f65e8d98ede2c94d2b1f1af780a87d11dfe674ad39a32966d0e23ed563d20c3e","ADR-043":"85de7df8d21029d6ef84b17b29db997fe318e95dba2187207c15cc3340b706f9","ADR-044":"f6e19d72435fb278213b117f08beae149c52695865a1c4bd45c36ebcbee44a44","ADR-045":"bb77b617437541b76a491b34ac823795879fba9a4a2912596f6919aaba7fbfa5","ADR-046":"42058f815bb3a0cda323014c430ac257157d9b8edd35c0cbf90956781aa2b446","ADR-047":"cab240e7dc9c1ab657fb02aaaf820d45cf69577a145679548624a1473512c012","ADR-048":"bdedb085c22082216d15421f21e2ea10e3afe61cce97080e9c9f180408b858c0","ADR-049":"af60be8e05e3968e605ccad0552d56ec96540b66e1529fc1337b94b43d54e601","ADR-050":"4f747a4e41435d2317e9ccbbfbcd0e4d12ff98a35c01b73c40d9c3b43829623a","ADR-051":"d3223c550c13f153b1a3ccd366898f693f12bcebf786349bc1352248cfeb5f06","ADR-052":"34dc063470d005834931ef0b845898560bdc78aa156e4ade27c7ee38579da852","ADR-053":"e462aa8d9adcc26125e5617a7ba7c0e9ceadb789fe4f3f403d5e79d260cc21e3","ADR-054":"68d96ac9338baccd6cfd30f9c6fcc5508ba3ab784431ce92546d992d5338bd3a","ADR-055":"4a141790900f750657098ef109ac0937efa8fb3263d5e5495dedf82ccd087533","ADR-056":"0a17f399c0ba0593cbe22684b337ad7af78ca5bcae4aaa90f02d00663c47f5db","ADR-057":"ae40de6cbe055b68d91f45fd3bf97740849a75da729dd54b01abb38f50921ef7","ADR-058":"0b5d21df283072c98822e489c7e530a1250ccc25f7eb396999be40d1d3fba5c3","ADR-059":"9b1a90a73b659e0f4f65ce5b90f74c57ed63b9c0743fa5f4f84c8c555b374914","ADR-060":"d17c70909fcf4a8a389dbb7ee5277076f5482ed3a691895ebede8213f7c700a5","ADR-061":"61e75602eae76ca965dbdf8637d55c9ade8b0f0767b54c604f1f8d01157bed9c","ADR-062":"836f8aca1511dae4e23d3abebc51e792522d32285059d9fb28dd044adf5e573a","ADR-063":"92e51703ac49ef0fd0c93f7ba57f4474bccec4815135b1879c1599d2d934f87c","ADR-064":"f07177c64a2a49b93f704a055ad545e65e7302c7f3e2710a6328b0fcf5ab5fb9","ADR-065":"8e3467f9ae949f71b80e3a5456fb13b4b2ef0212b758725cbddefa168d7795f5","ADR-066":"0c4c68a8a309f0869a188547466ac0f01ea769d51428f7201a980b1f5a865499","ADR-067":"4566f8ed549081c381927d6bb8ebdb0ac937c6aba60d09ea79bdeef53475acb4","ADR-068":"227d78b31d7b9ee14b5cf8c38a44b4afd68519cc20ab0e0510ad720182b10d0a","ADR-069":"60f681021f437432a1d998454f30905910eba235cc972dbab9c16fc1255b162f","ADR-070":"720f9ddb311c4ded3d7f1e247e69e4e59f5e69d4b307f3284cb58c0073057acf","ADR-071":"32be782c8bbe185758cf2516cfc4389ed078d0253f8c7133b0eaf950d3ff2b68","ADR-072":"32b2230cb9ef974553f1eeb77c251f5e08943e017461636d66bfdabcb7a8c217","ADR-073":"be787bdc6d4630492095b8642e90a2abf450ec16079e3d2aacd268d67cc590be","ADR-074":"24053a5ad104133f2c9e8ce3d8a133066ceb6fee367d3c6121c1d4ddf08610c2","ADR-075":"49d094186fe6143258d14adf32b014325767925cc4f5f439f368a1aac9168254","ADR-076":"0e1e83bc6754c08b4074d649875b9b1a58c50d5c5e9b098ecbf14de3a4129c0c","ADR-077":"77ec10579866c0d8ae12888cdbf6a6a33e3d43133c67bbe442a3f8a3ce298068","ADR-078":"ac35750afc3ba7a4b28ba185458d8ee5dd360ed9e4af3dc1941cdfafc6920f8d","ADR-079":"d97712523f23bee44aa48aceb17fb2e5822e734c09ba11057dcc4cb8e52f8e2d"}}

## 覆盖与路线结论

已逐卡阅读 T1.1–T1.15 的需求、输入、交付、不变量、修改范围、验收及回归，并从方案反查其用户流程。

| 范围 | 覆盖结论 |
|---|---|
| T1.1–T1.5 | 服务骨架、持久鉴权、TMDB、缓存首页、自定义 Stremio 及 App 接入均有承接；同配置多 manifest 的现有数据迁入仍缺具体覆盖，见 D006。 |
| T1.6–T1.10 | 来源、匹配映射、查找生命周期、测速与选线主线完整；实际播放上报、新历史写回及本地/直播保留的验收仍有缺口，见 D003–D005。 |
| T1.11–T1.15 | T4、Stremio HTTP、JSON 解析和配置导入均有独立交付及汇合；多结果重新解析时的线路身份规则仍需收口，见 D007。 |

DAG 无循环，依赖均指向前置卡。T1.11–T1.13 在 T1.10 后形成适配分支，T1.14 汇合两类 T4，T1.15 汇合 Stremio 和解析结果。匹配核心、lookup 生命周期、宿主接线和 App 界面按顺序交付，未来验收入口由对应开发卡创建。

按控制器实际的文件顺序、依赖和阶段门语义推演，当前第一张等待条件的卡为 T1.1，其等待条件是用户明确开发授权；该条件具有真实来源。授权后，各 T1 分支具备本机受控样本验收路径。外部真实接口按条件记录，当前规划没有把外网凭据作为全部本机分支的硬前提。

六张 T2 的依赖编号与顺延映射一致，T2.1 保留独立授权门。

## 新增已确认问题

### D003 — App 实际播放结果上报缺少接线与验收承接

- 类别：已确认缺口；严重度：P2；状态：OPEN。
- 需求：REQ-SERVICE-PLAY；方案 §5.5、§8.3。
- 位置：[方案第 372 行](/Users/eason/jp/application/docs/FOLLOWUP-PLAYBACK-SERVICE.md:372)、[T1.9 交付与验收](/Users/eason/jp/application/docs/TASKS.md:2935)、[T1.10 验收](/Users/eason/jp/application/docs/TASKS.md:2983)。
- 证据与反例：方案要求宿主在实际播放推进后上报 `playing`，失败时上报 `failed`。T1.8 承接服务端 reports；T1.9 六命令和 T1.10 播放场景均缺少 App 发出结果、服务收到结果的可观察要求。现有实际推进事件由前端 [state.ts](/Users/eason/jp/application/src/state.ts:244) 处理，网关登记成功本身无法证明播放推进。假设未来实现完全省略客户端上报，当前列出的短播、换线和服务 reports 单独检查仍可通过。
- 影响：用户实际播放成败无法进入服务端统计，列表统计与后续参考数据失真。
- 最小修订：在 T1.9/T1.10 明确实际播放事件到 reports 的承接；复用 T1.10 的短播和失败场景，断言服务收到正确 `line_id` 的 `playing`/`failed`，并核对上报失败时播放继续。

### D004 — 新服务播放缺少可信会话绑定与新历史写回闭环

- 类别：已确认缺口；严重度：P2；状态：OPEN。
- 需求：REQ-SERVICE-PLAY；方案 §8.3–§9；架构 C12、C16。
- 位置：[T1.9 交付](/Users/eason/jp/application/docs/TASKS.md:2935)、[T1.10 A2](/Users/eason/jp/application/docs/TASKS.md:3006)、[C16](/Users/eason/jp/application/docs/ARCHITECTURE.md:186)。
- 代码证据：当前 [open_playback](/Users/eason/jp/application/src-tauri/src/host.rs:517) 校验本地来源，并从 `resolution_contexts` 建立 `session_bindings`；现有 [resolve](/Users/eason/jp/application/src-tauri/src/host.rs:1631) 从匹配记录写入该 context；[save_history](/Users/eason/jp/application/src-tauri/src/host.rs:1696) 严格核对 Session、match、元数据身份、video 和来源。
- 因果与反例：T1.9 明确登记 Resolution 和打开网关，T1.10 同时移除 App 内匹配，计划尚未明确新服务来源与可信播放 context 如何接入上述链路。现写“观看记录能经新轮次续播”可以通过读取预先存在的记录后播放完成；即使新 Session 缺少可信绑定、保存返回 `SESSION_EXPIRED`，该场景仍可能通过。
- 影响：视频可播放，新观看进度却无法可靠保存，退出后续播位置丢失。
- 最小修订：T1.9 明确服务线路到来源校验、Resolution context、Session 绑定和历史身份的交接；T1.10 在现有短播中加入一条新记录的保存、SQLite 回读、重启后按新轮次恢复进度，并保留错误身份拒绝的相关检查。

### D005 — 服务断连时本地视频与直播仅验证入口

- 类别：已确认缺口；严重度：P2；状态：OPEN。
- 需求：REQ-SERVICE-PLAY；方案 §1、§8.4。
- 位置：[方案第 385 行](/Users/eason/jp/application/docs/FOLLOWUP-PLAYBACK-SERVICE.md:385)、[T1.10 A2](/Users/eason/jp/application/docs/TASKS.md:3006)。
- 证据与反例：方案要求服务不可达时本地视频和直播照常。T1.10 A2 当前仅要求“本地视频和直播抽查入口可用”。若未来公共初始化、设置或 Session 改动导致打开入口后等待服务、媒体请求失败，入口检查仍可通过。两类功能实际共用 Host 和网关，[open_live](/Users/eason/jp/application/src-tauri/src/host.rs:553)、[open_local](/Users/eason/jp/application/src-tauri/src/host.rs:639) 均位于受影响宿主内。
- 影响：服务故障可能连带阻断已承诺保留的本地播放能力。
- 最小修订：复用同一次服务不可达场景，分别短播一个本地文件和一个受控直播，观察媒体推进，并核对其播放路径独立于服务连接状态。

### D006 — 同一配置包含多个 manifest 的迁移缺少完整验收

- 类别：已确认缺口；严重度：P2；状态：OPEN。
- 需求：REQ-SERVICE-META；方案 §4.4、§9。
- 位置：[T1.4](/Users/eason/jp/application/docs/TASKS.md:2689)、[T1.5](/Users/eason/jp/application/docs/TASKS.md:2731)、[方案身份迁入要求](/Users/eason/jp/application/docs/FOLLOWUP-PLAYBACK-SERVICE.md:396)。
- 代码证据：当前 [StremioConfig](/Users/eason/jp/application/protocol/app.schema.json:3316) 接受 1–16 个 manifest；[metadata_identity](/Users/eason/jp/application/src-tauri/src/stremio.rs:1499) 同时保存配置 ID、各扩展的 addon key 和对应命名空间。服务方案的数据表以单个 `manifest_url` 描述元数据来源。
- 因果与反例：方案承诺当前自定义配置及历史、收藏保留；T1.4 验收使用一个代表性扩展及两个来源同名 ID，T1.5 使用一个自定义 Stremio 内容。未来实现若只迁入一个配置的首个 manifest，现写场景仍可通过，第二个扩展的历史和收藏会失去回源映射。
- 最小修订：交付约定明确原配置与各 addon 身份到服务来源的持久映射；在 T1.4/T1.5 的既有导入场景中加入“同配置两个 manifest”，核对两者内容、历史与收藏迁入、重复导入及重启回读。聚合或拆分方式可由实现选择。

### D007 — 刷新重新解析多线路时缺少身份保持规则与反例

- 类别：已确认缺口；严重度：P2；状态：OPEN。
- 需求：REQ-SERVICE-PLAY、REQ-SERVICE-PROVIDERS；方案 §5.5、§6.6–§6.7。
- 位置：[refresh 契约](/Users/eason/jp/application/docs/FOLLOWUP-PLAYBACK-SERVICE.md:207)、[T1.8 A1](/Users/eason/jp/application/docs/TASKS.md:2898)、[T1.13 A1](/Users/eason/jp/application/docs/TASKS.md:3130)。
- 证据：方案要求 refresh 返回同一 `line_id` 并保持线路身份。T4 play 支持多个 URL；Stremio 标准流响应中的名称和描述为可选字段，协议未要求每条 HTTP 流提供稳定 stream ID。[Stremio 官方 Stream Object 文档](https://raw.githubusercontent.com/Stremio/stremio-addon-sdk/master/docs/api/responses/stream.md)
- 因果与反例：当前计划尚未说明重解析结果重排、原线路消失或同标签多结果时如何定位原线路。假设按数组下标取刷新结果，旧 A 线路所在位置变为 B 后，B 地址仍可挂在 A 的 `line_id` 下；现有“刷新正确”代表样本缺少拒绝该实现的具体反例。
- 影响：用户选中的线路、刷新结果和成败统计发生错配，失败重试也可能实际切到另一线路。
- 最小修订：明确仅在原逻辑线路可唯一识别时保持原 ID 刷新；消失或歧义时返回明确过期状态，由 App 重查并保留进度。在现有刷新/适配场景加入重排和原线路消失样本。

## 旧问题与其他核对

| ID | 本轮核对 | 状态 |
|---|---|---|
| D001 | T1.1 已包含 `protocol/service.openapi.json`，交付物明确 health 与鉴权错误契约。 | FIXED |
| D002 | T1.5、T1.9、T1.10 已列入宿主命令注册、权限生成及相关契约检查范围；T1.15 允许来源字段需要的协议和转发调整。 | FIXED |

本轮新增已确认问题 5 项，解决 0 项。自动选线预算、全失败出口等已有不变量及错误处理承接，本轮未另增问题。根 workspace、server、crates 的源码摘要覆盖疑点经读取 `workflow.snapshot()` 排除。

## 实际检查

本审阅者实际执行：

- 通过项目 `.venv/bin/python -B`、argv 与 `shell=False` 调用 `scripts/task.py check`：退出码 0，`roadmap OK: 66 cards`。
- 使用现有 `workflow.cards()` 解析全部卡片：45 张 T0、15 张 T1、6 张 T2；T1 全部未完成，18 项验收的 suite 和报告路径唯一。
- 读取控制器的依赖、BLOCKED、授权和选卡语义，并结合当前授权记录推演：T1.1 与 T2.1 阶段门均关闭。
- 使用现有 `protected_hash()`、`adr_digests()` 取得并复核真实基线。
- 阅读相关设置、Stremio 身份、原生播放、历史写入、前端播放事件、IPC 注册和契约检查代码。
- `git diff --check`：退出码 0。
- 最终 Git 状态：仅 `docs/PROGRESS.md` 有外层追加记录，受审计划摘要保持一致。
- 打开 Stremio 官方流响应文档，核对刷新身份分析所依赖的协议事实。

业务实现、构建、服务运行、真实第三方接口及 App 播放验收均为 **NOT RUN**。本轮未修改文件、勾卡、启动开发或服务，未恢复依赖与构建缓存。

共享审查起点：UTC `02:43:41`；本报告最终时钟采样：UTC `02:52:48`，约 547 秒，截至该次采样，处于 15 分钟预算内。模型运行时实际标识、调用统计、token usage 与费用：**NOT AVAILABLE**。

DOC-REVIEW-VERDICT: BLOCKING
