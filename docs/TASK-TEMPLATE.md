# 任务卡格式

将下面的卡复制到 TASKS.md，并替换为当前真实需求。命令只是一种 Python 项目示例，不应照搬为其他技术栈的业务验收。
同时维护 TASKS.md 的需求覆盖表；需求行定位到原文章节或稳定 ID，卡内反向列出该需求对应的本卡验收。跨卡验收用完整 ID（如 T0.1/A1）避免不同卡的 A1 混淆；批准排除的要求须注明真实授权来源。

````markdown
### T0.1 首个内聚交付
- [ ] 完成
- 依赖：无
- 需求：REQ-001（`docs/SPEC.md` 对应章节）→ A1
- 范围：`src/`、`tests/`
- 环境：（可选）必需执行环境与非阻断记录环境，如 `macos-arm64 必需；windows/linux 记录`
- 阶段门：（可选）需要人类另行授权时写明授权内容；无此要求时删除本行
- 交付物：描述输入、行为、输出及对应架构契约。
- 验收：
```json
[
  {
    "id": "A1",
    "description": "用固定输入核对输出、边界及明确错误",
    "argv": ["{python}", "-B", "tests/acceptance/t0_1.py", "--junit", "evidence/acceptance/T0.1-A1.xml"],
    "kind": "test",
    "report": "junit:evidence/acceptance/T0.1-A1.xml",
    "cases": ["REQ-001/valid-input", "REQ-001/rejects-empty", "REQ-002/optional-env"],
    "skippable": ["REQ-002/optional-env"],
    "timeout_seconds": 600
  }
]
```
````

卡片验收优先用 JUnit 报告并列出 `cases`：每个用例 ID 以需求 ID 开头，JUnit testcase 的 name 与之完全一致；控制器要求每个用例出现且通过，只有 `skippable` 中声明的条件项（如本机不可达环境）可以 skipped。`cases` 在规划时冻结，开发者不能增删用例来改变覆盖。
JUnit 报告路径在全部卡片中唯一，统一写 `evidence/acceptance/<卡号>-<验收ID>.xml`。`task.py check` 核对每卡全部验收超时加完整 verify 超时不超过 `session_seconds × max_sessions`，单项超过一个会话时提示须跨会话续跑。
可读性：验收描述分条写“输入/环境 → 期望结果 → 会被拒绝的错误实现”，不用斜杠串联多个条件；细节放进 `cases`，一个 case 对应一个可独立失败的行为。
独立审查类验收（如最终独立验收）把审阅报告提交到 `docs/reviews/<卡号>/round-<N>.md`，正文写所审提交 SHA 与源码摘要，末行 `<类型>-VERDICT: CLEAR|BLOCKING`；验收脚本取 N 最大的一轮，核对提交与本次构建一致、末行为 CLEAR。
测试类型 `kind=test` 的 report 支持 `unittest`、`cargo`、`pytest`、`junit:evidence/test-results/result.xml`。
CLI/报告断言可用 `kind=command`，argv 应调用真实断言脚本；退出0必须意味着完整断言成立，不用 echo/true 冒充交付验收。
验收描述写明输入、期望结果和会被拒绝的错误行为；性能验收保留指标、阈值、单位、负载/规模、环境、测量方法、适用阶段和合格断言，按来源区分硬门槛、目标与校准。涉及真实集群时须验收 Docker 服务成员部署、宿主仅客户端/报告/监管、无原生服务 fallback、各容器硬内存上限合计与宿主采样 RSS 合并预算及自产资源清理。原入口/解释器/锁/证据必须对应已绑定项目，共享重锁保持全局路径。
夹具或组件验收应说明边界，最终真实接线由具名卡和验收覆盖，不能用局部测试冒充端到端覆盖。用户的真实项目路径、指标和运行限制用于建立契约，不挪作示例分析或测试夹具。
`{python}` 会替换为启动工作流的解释器。卡片里写 `{python}`，不写 `python3`、`python` 或 `py`。文档、日志、报告路径采用项目内 `/` 路径。Windows 原生程序或解释器使用明确 argv，不隐式拼接 batch/shell 命令。
复合命令放到项目脚本，资源包装器调用该脚本；其内部不能嵌套 run.py/verify.py。等待要有 deadline，服务成功或失败都须清理。
