# M10: LeRobot Profile System

**Layer**: 4 — Attribution Bar
**Depends on**: M09 (attribution bar)
**Effort**: 1 week

## Revision 2026-08-13 — confirmed decisions (with student)

1. **Parquet 解析用 Python 子进程桥**（用户优先决策）: 发布后目标用户
   （dora/LeRobot 用户）必然有 Python + pyarrow（lerobot 的强制依赖），
   手写 Rust Parquet 解析残缺且工期不可行。后端 spawn `python3
   backend/scripts/lerobot_reader.py`，JSON stdin/stdout 协议，超时保护；
   pyarrow 缺失时优雅降级（结构化错误 + 安装提示），不崩溃。
2. **链粒度: 每帧一条链**。SensorFrame←observation.state、Prompt←task 描述
   （meta/tasks.parquet，缺失则 "Task {n}"）、ParsedAction←action 向量。
   LLM 回复/执行结果步骤缺失 → UI 显示 "Not available in this dataset"
   占位，不伪造。
3. **UI: 归因面板内扩展**（不新建页面）。数据源下拉启用 LeRobot 项 →
   面板内显示路径输入、Scan、数据集信息、配置选择（含自动检测）、
   episode 选择 → Load → 复用 M09 链条/详情卡。
4. **Profile 优先级: B601 优先**（本机有真实数据可验证），SO-100/GEN72
   作为模板配置一并提供；真实验证以 B601 为准。
5. **诚实性类型调整**（影响 M09，同步更新）:
   - `AttributionStep::ParsedAction.confidence: f32` → `Option<f32>`，
     载荷版本升 v2，重生成演示 .drec 文件
   - `AttributionChainSummary.success: bool` → `Option<bool>`，
     无执行结果时前端显示中性灰
6. **"Show in 3D"**（lerobot 源）: 发出 action 向量 → 视口切回 Live 模式，
   机械臂摆出该帧姿态（不走 .drec seek）。
7. **分页**: 每 episode 帧数可达 ~900（B601 实测 897 帧/集），链按页加载，
   每页 200 条。

## Purpose

Support loading LeRobot-format datasets (Parquet files) into the
attribution bar. A profile YAML maps robot-specific field names to
the attribution bar's fixed schema, so switching robots is just
swapping a profile file.

**对 dora 生态的意义**: M09 解释部署侧"机器人为什么这么做"，M10 让用户
检查训练它的数据集（"它学到的行为来自哪些数据"）——归因闭环的另一半。
LeRobot 是 HF 社区事实标准，读开放格式使 Studio 成为独立的机器人数据
检查工具（不跑 dataflow 也能用），是采纳杠杆；Profile 机制演示面向
异构机器人的扩展性路径。

## 真实数据事实（B601 pilot，~/.cache/huggingface/lerobot/my_org/b601_pilot_v1）

- 5 个 episode（`data/episode_000000..4.parquet`），每集 ~897 行，30Hz，~29.8s
- 列: `action: List<float>`(7 关节)、`observation.state: List<float>`、
  `timestamp: float`(集内相对秒，从 0 起)、`frame_index/episode_index/index/task_index: int64`
- `meta/tasks.parquet`: 1 行 — task_index 0 → "Pick up the red cube and
  place it completely inside the tray."（真实任务描述 → Prompt 文本）
- 图像在独立视频文件（meta 中有 videos/observation.images.* 引用），
  parquet 内无图像列 → 缩略图显示占位，不伪造
- 另有 v2 布局数据集（b601_pick_red_cube_v1.bak，chunk-000/file-*.parquet）
  可用于布局兼容验证

## Deliverables

### D1: LeRobot dataset reader (Python 桥)

- `backend/scripts/lerobot_reader.py` — JSON stdin/stdout 协议，子命令:
  - `scan <path>` → `{name, layout: v1|v2, columns, episodes: [{index, rows,
    startTs, endTs}], tasks: {task_index: description}, hasImageColumns}`
  - `frames <path> <episode> <offset> <limit>` → `{frames: [{frameIndex,
    timestamp, taskIndex, action, state}], total}`
- 支持 v1（`data/episode_*.parquet`）与 v2（`data/chunk-*/file-*.parquet`，
  按 episode_index 分组）两种布局
- `backend/src/lerobot.rs` — spawn python3（CARGO_MANIFEST_DIR 相对路径
  定位脚本）、超时（30s）、stderr 捕获、pyarrow 缺失的结构化错误
- tasks.parquet 任务描述列名不固定（B601 实测为 pandas 遗留名
  `__index_level_0__`）→ 取"非 task_index 的列"作为描述
- 缩略图: 仅当 parquet 有 `observation.images.*` 列时提取首帧 JPEG
  （base64）；B601 无 → 占位

### D2: Profile YAML 解析器

New file: `backend/src/profile.rs`

- 极简行级 YAML 解析（沿用 dataflow_builder 的零依赖做法），schema:
  ```yaml
  robot: B601
  fields:
    state: [observation.state, observations/state, obs.state]
    action: [action]
    task: [task_index]
    timestamp: [timestamp]
    frame_index: [frame_index]
  joint_mapping:
    arm_joints: [0, 1, 2, 3, 4, 5]
    gripper: 6
  ```
- `profiles/` 目录（提交）: `lerobot_profile_b601.yaml`、
  `lerobot_profile_so100.yaml`、`lerobot_profile_gen72.yaml` + `README.md`
- `ProfileManager::list_available()` 扫描 profiles/ 目录
- 别名匹配: 每个语义字段多候选列名，按数据实际列名首个命中优先
- Auto-detect: 按"语义字段命中率"给每个 profile 打分，返回最佳建议

### D3: Profile-aware attribution

- `AttributionExtractor::from_lerobot(dataset_path, profile, episode, offset,
  limit) -> Result<LerobotChains>`，产出与 M09 相同的 `AttributionChain`
- 步骤映射（每帧）:
  1. `SensorFrame {topic: "lerobot/observation.state", width: state_len,
     height: 1, encoding: "float32"}` — 元数据，诚实
  2. `Prompt {text: task 描述, token_count: 词数}`
  3. `ParsedAction {action_type: "joint_target", vector: action,
     confidence: None}`（置信度未记录 → null）
  - LLM 回复 / 执行结果: 不生成步骤，UI 占位显示 Not available
- 时间戳: `(timestamp - episode_start) * 1e9` 纳秒归一化
- 缺失字段: 该步骤不生成（占位），不报错

### D4: Dataset browser (归因面板内扩展)

`frontend/src/components/AttributionBar.vue` 扩展（不新建页面）:

- 数据源下拉启用 "LeRobot dataset" → 面板体切换为 LeRobot 区:
  路径输入 + Scan 按钮 → 数据集信息（名称/布局/episode 数/列）+ 配置
  选择（下拉 + Auto-detect 按钮）+ episode 列表（索引/帧数/时长/缩略图
  或占位）→ Load → 链条/详情卡复用
- 链分页控件（每页 200 条，‹ › 翻页）
- 详情卡缺失步骤（LLM 回复/执行结果）显示灰色 "Not available in this
  dataset" 占位行；confidence null → "n/a"
- 无执行结果的链 → 中性灰（不显示成功/失败色）
- "Show in 3D": lerobot 源 → emit `apply-action(vector)` →
  VisualizationView 切 Live 模式并应用前 6 关节到视口

### D5: Profile 验证

- B601: 真实数据全链路验证（scan → frames → 归因链 → UI）
- SO-100 / GEN72: 模板配置 + 合成数据集验证（别名匹配、字段缺失占位）
- 自动化测试含 profile 解析、别名匹配、auto-detect 评分

## Acceptance Criteria

- [ ] Load LeRobot 数据集目录（v1 与 v2 布局），显示 episode 列表
- [ ] B601 真实数据: scan 出 5 episodes + 任务描述，frames 出 action/state
- [ ] 每帧一条归因链，时间戳归一化正确（0..~29.8s）
- [ ] Prompt 来自真实任务描述 "Pick up the red cube..."
- [ ] 缺失步骤（LLM 回复/执行结果）显示 Not available 占位，不伪造
- [ ] Profile 切换 + 字段别名匹配 + auto-detect 建议正确 profile
- [ ] 分页加载（每页 200 链）正常
- [ ] "Show in 3D" 应用 action 向量到视口（Live 模式）
- [ ] pyarrow 缺失时优雅降级提示
- [ ] `cargo test` 全绿（含跳过真实数据的集成测试）；前端构建通过
- [ ] Profile YAML 格式文档化（profiles/README.md）

## Exposed Interfaces

```rust
// backend/src/lerobot.rs
pub struct LerobotStatus { python_available: bool, pyarrow_available: bool, message: String }
pub struct DatasetInfo { name, layout, columns, episodes: Vec<EpisodeInfo>, tasks: HashMap<u32, String>, has_image_columns: bool }
pub struct EpisodeInfo { index: u32, rows: usize, start_ns: u64, end_ns: u64 }
pub struct FrameData { frame_index, timestamp_ns, task_index: Option<u32>, action: Vec<f32>, state: Vec<f32> }
pub struct LerobotChains { chains: Vec<AttributionChain>, total: usize }

// backend/src/profile.rs
pub struct RobotProfile { robot_name, fields: FieldAliases, joint_mapping: JointMapping }
pub struct ProfileManager { ... } // list_available, load, autodetect(columns) -> (profile, score)

// API (main.rs)
GET  /api/lerobot/status
POST /api/lerobot/scan          {path}
POST /api/lerobot/frames        {path, episode, offset, limit}
GET  /api/lerobot/profiles
POST /api/lerobot/autodetect    {path}
POST /api/lerobot/attribution   {path, profile, episode, offset, limit}
```

## 测试策略

- profile.rs 单测: YAML 解析、别名匹配（observation.state ↔
  observations/state ↔ obs.state）、auto-detect 评分 — 纯 Rust，无 Python
- lerobot.rs 单测: 用脚本自身写出的合成迷你数据集（v1 + v2 布局，
  pyarrow 写 parquet），跑 scan/frames/attribution；python3/pyarrow 缺失时
  测试跳过并打印原因
- 真实数据集成测试: B601 路径存在时验证 scan/frames/attribution，
  否则跳过（`#[ignore]` 或运行时跳过）
- M09 回归: confidence Option 化的编解码测试更新，演示 .drec 重生成

## 风险

| 风险 | 缓解 |
|------|------|
| pyarrow 缺失（无 Python 环境） | status 端点 + 结构化错误 + 安装提示 |
| tasks.parquet 缺失/格式不同 | 回退 "Task {n}" 模板 |
| 大 episode（数千帧） | 分页 200/页 + 帧级懒加载 |
| v2 布局差异 | scan 按 episode_index 分组，实测 .bak 数据集 |
| M09 载荷版本升级 | 无外部 v1 文件，重生成演示文件即可 |
