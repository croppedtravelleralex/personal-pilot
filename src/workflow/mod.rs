use std::{fs, path::Path};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_WORKFLOW_STATE_PATH: &str = "RUN_STATE.json";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStage {
    Plan,
    Execute,
    Verify,
    BugScan,
    BugFix,
    DocSync,
    CommitPush,
    Cooldown,
    Blocked,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowSuggestionKind {
    Feature,
    BugScan,
    BugFix,
    DocSync,
    Refactor,
    Performance,
    Test,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowSuggestion {
    pub title: String,
    pub priority: u8,
    pub rationale: String,
    pub kind: WorkflowSuggestionKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowExecutionState {
    pub project: String,
    pub loop_enabled: bool,
    pub loop_iteration: u64,
    pub stage: WorkflowStage,
    pub bug_cycle_interval: u64,
    pub completed_since_bug_cycle: u64,
    pub consecutive_failures: u64,
    pub current_focus: String,
    pub current_objective: String,
    pub last_result_summary: String,
    pub next_action_hint: String,
    pub next_suggestions: Vec<WorkflowSuggestion>,
    pub last_executed_actions: Vec<WorkflowActionRecord>,
    pub blocked_reason: String,
    pub cooldown_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowActionRecord {
    pub title: String,
    pub kind: WorkflowSuggestionKind,
    pub status: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkflowDispatchResult {
    pub executed: Vec<WorkflowActionRecord>,
}

impl WorkflowExecutionState {
    pub fn new(project: impl Into<String>) -> Self {
        Self {
            project: project.into(),
            loop_enabled: false,
            loop_iteration: 0,
            stage: WorkflowStage::Plan,
            bug_cycle_interval: 3,
            completed_since_bug_cycle: 0,
            consecutive_failures: 0,
            current_focus: "建立自动执行工作流状态机骨架".to_string(),
            current_objective: "初始化 workflow state 文件与阶段枚举".to_string(),
            last_result_summary: "尚未开始自动循环".to_string(),
            next_action_hint: "先进入 plan 阶段，读取目标文档并生成建议".to_string(),
            next_suggestions: default_suggestions_for_stage(WorkflowStage::Plan),
            last_executed_actions: Vec::new(),
            blocked_reason: String::new(),
            cooldown_seconds: 30,
        }
    }

    pub fn should_enter_bug_cycle(&self) -> bool {
        self.should_run_bug_scan_now()
    }

    pub fn advance_after_success(&mut self) {
        self.loop_iteration += 1;
        self.consecutive_failures = 0;
        self.blocked_reason.clear();
        self.completed_since_bug_cycle += 1;
        self.stage = if self.should_enter_blocked() {
            WorkflowStage::Blocked
        } else if self.should_enter_bug_cycle() {
            WorkflowStage::BugScan
        } else {
            match self.stage {
                WorkflowStage::Plan => WorkflowStage::Execute,
                WorkflowStage::Execute => WorkflowStage::Verify,
                WorkflowStage::Verify => WorkflowStage::DocSync,
                WorkflowStage::BugScan => WorkflowStage::BugFix,
                WorkflowStage::BugFix => {
                    self.completed_since_bug_cycle = 0;
                    WorkflowStage::CommitPush
                }
                WorkflowStage::DocSync => WorkflowStage::Cooldown,
                WorkflowStage::CommitPush => WorkflowStage::Cooldown,
                WorkflowStage::Cooldown => WorkflowStage::Plan,
                WorkflowStage::Blocked => WorkflowStage::Plan,
            }
        };
        self.next_suggestions = default_suggestions_for_stage(self.stage);
    }

    pub fn mark_failure(&mut self, summary: impl Into<String>) {
        self.consecutive_failures += 1;
        self.last_result_summary = summary.into();
        if self.should_enter_blocked() {
            self.mark_blocked("连续失败次数过高，进入 blocked 等待恢复");
        } else {
            self.stage = WorkflowStage::BugScan;
            self.next_suggestions = default_suggestions_for_stage(self.stage);
        }
    }

    pub fn should_enter_blocked(&self) -> bool {
        self.consecutive_failures >= 3
    }

    pub fn should_run_bug_scan_now(&self) -> bool {
        self.completed_since_bug_cycle >= self.bug_cycle_interval || self.consecutive_failures > 0
    }

    pub fn mark_blocked(&mut self, reason: impl Into<String>) {
        self.blocked_reason = reason.into();
        self.stage = WorkflowStage::Blocked;
        self.next_action_hint = "先解除阻塞，再恢复到 plan".to_string();
        self.next_suggestions = default_suggestions_for_stage(self.stage);
    }

    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let raw = fs::read_to_string(path)
            .with_context(|| format!("failed to read workflow state from {}", path.display()))?;
        Self::from_json_str(&raw)
            .with_context(|| format!("failed to parse workflow state from {}", path.display()))
    }

    pub fn save(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create workflow state dir {}", parent.display()))?;
        let text =
            serde_json::to_string_pretty(self).context("failed to serialize workflow state")?;
        fs::write(path, text)
            .with_context(|| format!("failed to write workflow state to {}", path.display()))?;
        Ok(())
    }

    pub fn ensure_default_state_file(path: impl AsRef<Path>, project: &str) -> Result<Self> {
        let path = path.as_ref();
        if path.exists() {
            match Self::load(path) {
                Ok(state) => return Ok(state),
                Err(_) => {
                    let raw = fs::read_to_string(path).with_context(|| {
                        format!(
                            "failed to read legacy workflow state from {}",
                            path.display()
                        )
                    })?;
                    let migrated = Self::from_json_str(&raw).unwrap_or_else(|_| Self::new(project));
                    migrated.save(path)?;
                    return Ok(migrated);
                }
            }
        }
        let state = Self::new(project);
        state.save(path)?;
        Ok(state)
    }

    fn from_json_str(raw: &str) -> Result<Self> {
        match serde_json::from_str::<Self>(raw) {
            Ok(state) => Ok(state),
            Err(_) => Self::from_legacy_value(
                serde_json::from_str(raw).context("failed to parse workflow json value")?,
            ),
        }
    }

    fn from_legacy_value(value: Value) -> Result<Self> {
        let stage = match value
            .get("nextRoundType")
            .and_then(|v| v.as_str())
            .unwrap_or("plan")
        {
            "plan" => WorkflowStage::Plan,
            "build" => WorkflowStage::Execute,
            "verify" => WorkflowStage::Verify,
            "summarize" => WorkflowStage::DocSync,
            _ => WorkflowStage::Plan,
        };
        let next_suggestions = value
            .get("nextRecommendedActions")
            .and_then(|v| v.as_array())
            .map(|items| {
                items
                    .iter()
                    .enumerate()
                    .filter_map(|(idx, item)| {
                        item.as_str().map(|title| WorkflowSuggestion {
                            title: title.to_string(),
                            priority: (idx + 1) as u8,
                            rationale: "从旧 RUN_STATE.json 迁移而来".to_string(),
                            kind: WorkflowSuggestionKind::Feature,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_else(|| default_suggestions_for_stage(stage));

        Ok(Self {
            project: value
                .get("project")
                .and_then(|v| v.as_str())
                .unwrap_or("PersonaPilot")
                .to_string(),
            loop_enabled: value.get("schedulerStatus").and_then(|v| v.as_str()) == Some("running"),
            loop_iteration: value
                .get("currentRound")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            stage,
            bug_cycle_interval: 3,
            completed_since_bug_cycle: 0,
            consecutive_failures: value
                .get("failureCount")
                .and_then(|v| v.as_u64())
                .unwrap_or(0),
            current_focus: value
                .get("currentFocus")
                .and_then(|v| v.as_str())
                .unwrap_or("迁移旧执行状态")
                .to_string(),
            current_objective: value
                .get("currentObjective")
                .and_then(|v| v.as_str())
                .unwrap_or("初始化新的工作流状态机")
                .to_string(),
            last_result_summary: value
                .get("lastVerificationResult")
                .and_then(|v| v.as_str())
                .unwrap_or("从旧 RUN_STATE.json 迁移")
                .to_string(),
            next_action_hint: value
                .get("lastSchedulerDecision")
                .and_then(|v| v.as_str())
                .unwrap_or("下一步进入计划阶段")
                .to_string(),
            next_suggestions,
            last_executed_actions: Vec::new(),
            blocked_reason: String::new(),
            cooldown_seconds: 30,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WorkflowDocumentContext {
    pub vision: String,
    pub current_direction: String,
    pub todo: String,
}

impl WorkflowDocumentContext {
    pub fn load_from_files(
        vision_path: impl AsRef<Path>,
        current_direction_path: impl AsRef<Path>,
        todo_path: impl AsRef<Path>,
    ) -> Result<Self> {
        Ok(Self {
            vision: fs::read_to_string(vision_path.as_ref())
                .with_context(|| format!("failed to read {}", vision_path.as_ref().display()))?,
            current_direction: fs::read_to_string(current_direction_path.as_ref()).with_context(
                || {
                    format!(
                        "failed to read {}",
                        current_direction_path.as_ref().display()
                    )
                },
            )?,
            todo: fs::read_to_string(todo_path.as_ref())
                .with_context(|| format!("failed to read {}", todo_path.as_ref().display()))?,
        })
    }
}

pub fn generate_dynamic_suggestions(
    stage: WorkflowStage,
    ctx: &WorkflowDocumentContext,
) -> Vec<WorkflowSuggestion> {
    let mut suggestions = Vec::new();
    let todo = ctx.todo.to_lowercase();
    let direction = ctx.current_direction.to_lowercase();
    let vision = ctx.vision.to_lowercase();

    if direction.contains("trust score") {
        suggestions.push(suggestion(
            "继续推进 trust score 核心化",
            1,
            "CURRENT_DIRECTION 明确要求继续把 proxy selection 收敛到 trust score 核心表达",
            WorkflowSuggestionKind::Feature,
        ));
    }
    if direction.contains("verify") {
        suggestions.push(suggestion(
            "推进 verify / smoke / batch verify 质量闭环",
            2,
            "当前方向要求把 verify 信号统一成更稳定的质量闭环",
            WorkflowSuggestionKind::Feature,
        ));
    }
    if todo.contains("写放大") || direction.contains("写放大") {
        suggestions.push(suggestion(
            "治理高并发写放大与状态竞争",
            3,
            "TODO 与 CURRENT_DIRECTION 都把写放大控制列为当前重点",
            WorkflowSuggestionKind::Performance,
        ));
    }
    if direction.contains("文档") || todo.contains("同步 current_") {
        suggestions.push(suggestion(
            "继续同步 CURRENT_*/TODO/STATUS 口径",
            4,
            "当前阶段强调文档、策略、代码主链要保持同一口径",
            WorkflowSuggestionKind::DocSync,
        ));
    }
    if vision.contains("可替换执行引擎") || vision.contains("artifact") {
        suggestions.push(suggestion(
            "补执行引擎边界与 artifact 策略",
            5,
            "VISION 强调可替换执行引擎与长期运行下的结果管理能力",
            WorkflowSuggestionKind::Refactor,
        ));
    }

    if stage == WorkflowStage::BugScan {
        return vec![
            suggestion(
                "查找 bug",
                1,
                "bug 环固定第一项为查找问题",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "修复 bug",
                2,
                "bug 环固定第二项为修复问题",
                WorkflowSuggestionKind::BugFix,
            ),
        ];
    }

    if suggestions.is_empty() {
        default_suggestions_for_stage(stage)
    } else {
        suggestions.sort_by_key(|s| s.priority);
        suggestions.truncate(5);
        suggestions
    }
}

pub fn refresh_dynamic_suggestions(
    state: &mut WorkflowExecutionState,
    vision_path: impl AsRef<Path>,
    current_direction_path: impl AsRef<Path>,
    todo_path: impl AsRef<Path>,
) -> Result<()> {
    let ctx =
        WorkflowDocumentContext::load_from_files(vision_path, current_direction_path, todo_path)?;
    state.next_suggestions = generate_dynamic_suggestions(state.stage, &ctx);
    Ok(())
}

fn append_execution_log(entries: &[WorkflowActionRecord]) -> Result<()> {
    if entries.is_empty() {
        return Ok(());
    }
    let path = Path::new("EXECUTION_LOG.md");
    let mut existing = if path.exists() {
        fs::read_to_string(path)?
    } else {
        String::new()
    };
    if !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(
        "
## Workflow Action Dispatch

",
    );
    for entry in entries {
        existing.push_str(&format!(
            "- {} [{}]: {}
",
            entry.title,
            match entry.kind {
                WorkflowSuggestionKind::Feature => "feature",
                WorkflowSuggestionKind::BugScan => "bug_scan",
                WorkflowSuggestionKind::BugFix => "bug_fix",
                WorkflowSuggestionKind::DocSync => "doc_sync",
                WorkflowSuggestionKind::Refactor => "refactor",
                WorkflowSuggestionKind::Performance => "performance",
                WorkflowSuggestionKind::Test => "test",
            },
            entry.note
        ));
    }
    fs::write(path, existing)?;
    Ok(())
}

pub fn dispatch_top_suggestions(
    state: &mut WorkflowExecutionState,
    max_actions: usize,
) -> WorkflowDispatchResult {
    let mut executed = state
        .next_suggestions
        .iter()
        .take(max_actions)
        .map(|suggestion| WorkflowActionRecord {
            title: suggestion.title.clone(),
            kind: suggestion.kind,
            status: "logged".to_string(),
            note: format!(
                "已执行最小真实动作：将建议写入 EXECUTION_LOG.md；原因：{}",
                suggestion.rationale
            ),
        })
        .collect::<Vec<_>>();
    if let Err(err) = append_execution_log(&executed) {
        for item in &mut executed {
            item.status = "log_failed".to_string();
            item.note = format!("写入 EXECUTION_LOG.md 失败：{}", err);
        }
    }
    state.last_executed_actions = executed.clone();
    WorkflowDispatchResult { executed }
}

pub fn run_minimal_cycle_step(state: &mut WorkflowExecutionState) {
    match state.stage {
        WorkflowStage::Plan => {
            state.current_focus = "对齐目标文档并生成本轮建议".to_string();
            state.current_objective =
                "读取 VISION/CURRENT_DIRECTION/TODO 后确定前两项动作".to_string();
            state.last_result_summary = "已完成 plan 阶段，生成下一阶段建议".to_string();
            state.next_action_hint = "进入 execute，默认执行前两个建议".to_string();
            state.advance_after_success();
        }
        WorkflowStage::Execute => {
            let dispatch = dispatch_top_suggestions(state, 2);
            state.current_focus = "执行建议前两项".to_string();
            state.current_objective = "完成当前最优先的两个动作并补最小必要验证".to_string();
            state.last_result_summary = format!(
                "已完成 execute 阶段，已分发 {} 个动作，进入 verify 检查结果稳定性",
                dispatch.executed.len()
            );
            state.next_action_hint = "进入 verify，优先跑定向测试和一致性检查".to_string();
            state.advance_after_success();
        }
        WorkflowStage::Verify => {
            state.current_focus = "验证本轮结果并检查是否需要 bug 环".to_string();
            state.current_objective = "完成测试、口径一致性检查与风险扫描".to_string();
            state.last_result_summary = "已完成 verify 阶段，准备同步文档".to_string();
            state.next_action_hint = "进入 doc_sync，更新 TODO/STATUS/PROGRESS".to_string();
            state.advance_after_success();
        }
        WorkflowStage::BugScan => {
            state.current_focus = "进入 bug 环并锁定问题".to_string();
            state.current_objective = "优先定位最值得修复的问题".to_string();
            state.last_result_summary = "已完成 bug_scan，已定位问题，进入 bug_fix".to_string();
            state.next_action_hint = "进入 bug_fix，最小修复并补测试".to_string();
            state.advance_after_success();
        }
        WorkflowStage::BugFix => {
            state.current_focus = "修复 bug 并锁测试".to_string();
            state.current_objective = "完成最小修复，准备提交".to_string();
            state.last_result_summary = "已完成 bug_fix，准备 commit/push".to_string();
            state.next_action_hint = "进入 commit_push，提交稳定成果".to_string();
            state.advance_after_success();
        }
        WorkflowStage::DocSync => {
            state.current_focus = "同步文档与当前阶段状态".to_string();
            state.current_objective = "更新 TODO/STATUS/PROGRESS 与执行日志".to_string();
            state.last_result_summary = "已完成 doc_sync，进入 cooldown".to_string();
            state.next_action_hint = "进入 cooldown，短暂冷却后回到 plan".to_string();
            state.advance_after_success();
        }
        WorkflowStage::CommitPush => {
            state.current_focus = "提交当前稳定成果".to_string();
            state.current_objective = "commit 当前轮结果，并按条件评估 push".to_string();
            state.last_result_summary = "已完成 commit_push，进入 cooldown".to_string();
            state.next_action_hint = "进入 cooldown，然后回到 plan".to_string();
            state.advance_after_success();
        }
        WorkflowStage::Cooldown => {
            state.current_focus = "冷却并准备下一轮".to_string();
            state.current_objective = format!(
                "冷却 {} 秒后结束当前小循环，回到 plan",
                state.cooldown_seconds
            );
            state.last_result_summary = format!(
                "已完成 cooldown（{} 秒），下一轮重新进入 plan",
                state.cooldown_seconds
            );
            state.next_action_hint = "重新读取文档并生成新建议".to_string();
            state.advance_after_success();
        }
        WorkflowStage::Blocked => {
            state.current_focus = "解除阻塞".to_string();
            state.current_objective = if state.blocked_reason.is_empty() {
                "先识别阻塞，再回到 plan".to_string()
            } else {
                format!("先处理阻塞原因：{}", state.blocked_reason)
            };
            state.last_result_summary = "blocked 已切回 plan，等待恢复推进".to_string();
            state.next_action_hint = "回到 plan 重新评估".to_string();
            state.advance_after_success();
        }
    }
}

pub fn tick_workflow_file(path: impl AsRef<Path>, project: &str) -> Result<WorkflowExecutionState> {
    let path = path.as_ref();
    let mut state = WorkflowExecutionState::ensure_default_state_file(path, project)?;
    run_minimal_cycle_step(&mut state);
    let _ = refresh_dynamic_suggestions(&mut state, "VISION.md", "CURRENT_DIRECTION.md", "TODO.md");
    state.save(path)?;
    Ok(state)
}

pub fn run_minimal_cycle_steps(
    path: impl AsRef<Path>,
    project: &str,
    steps: usize,
) -> Result<WorkflowExecutionState> {
    let path = path.as_ref();
    let mut state = WorkflowExecutionState::ensure_default_state_file(path, project)?;
    for _ in 0..steps {
        run_minimal_cycle_step(&mut state);
    }
    let _ = refresh_dynamic_suggestions(&mut state, "VISION.md", "CURRENT_DIRECTION.md", "TODO.md");
    state.save(path)?;
    Ok(state)
}

pub fn default_suggestions_for_stage(stage: WorkflowStage) -> Vec<WorkflowSuggestion> {
    match stage {
        WorkflowStage::Plan => vec![
            suggestion(
                "读取目标文档并重新排序下一阶段事项",
                1,
                "先对齐 VISION/CURRENT_DIRECTION/TODO，避免跑偏",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "生成 3–5 个下一阶段建议",
                2,
                "为执行前两个动作提供稳定输入",
                WorkflowSuggestionKind::Feature,
            ),
            suggestion(
                "同步当前状态与目标口径",
                3,
                "减少 STATUS/TODO/PROGRESS 漂移",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "检查是否需要进入 bug 环",
                4,
                "控制 flaky、warning 和状态漂移",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "准备本轮执行上下文",
                5,
                "为 Execute 阶段降低切换成本",
                WorkflowSuggestionKind::Refactor,
            ),
        ],
        WorkflowStage::Execute => vec![
            suggestion(
                "执行建议第 1 项",
                1,
                "默认推进当前最优先事项",
                WorkflowSuggestionKind::Feature,
            ),
            suggestion(
                "执行建议第 2 项",
                2,
                "保持双任务推进节奏",
                WorkflowSuggestionKind::Feature,
            ),
            suggestion(
                "补最小必要测试",
                3,
                "避免推进后没有验证锁定",
                WorkflowSuggestionKind::Test,
            ),
            suggestion(
                "补必要文档口径",
                4,
                "防止代码与文档脱节",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "记录本轮产出",
                5,
                "为后续 verify/commit 提供依据",
                WorkflowSuggestionKind::DocSync,
            ),
        ],
        WorkflowStage::Verify => vec![
            suggestion(
                "跑定向测试与一致性检查",
                1,
                "验证刚完成的两项动作是否稳定",
                WorkflowSuggestionKind::Test,
            ),
            suggestion(
                "检查 explain/preview/task/status 口径",
                2,
                "优先发现状态漂移",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "检查 warning / flaky 信号",
                3,
                "提前识别不稳定点",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "记录验证结果",
                4,
                "为 bug 环或 commit 提供结论",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "决定是否进入 bug 环",
                5,
                "控制推进质量",
                WorkflowSuggestionKind::BugScan,
            ),
        ],
        WorkflowStage::BugScan => vec![
            suggestion(
                "查找 bug",
                1,
                "bug 环第一优先项固定为查找问题",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "锁定最值得修的 bug",
                2,
                "避免同时修多个低价值噪音",
                WorkflowSuggestionKind::BugFix,
            ),
        ],
        WorkflowStage::BugFix => vec![
            suggestion(
                "修复 bug",
                1,
                "bug 环第二步固定为最小修复",
                WorkflowSuggestionKind::BugFix,
            ),
            suggestion(
                "补测试锁住修复",
                2,
                "防止回归",
                WorkflowSuggestionKind::Test,
            ),
            suggestion(
                "复查是否还有连带问题",
                3,
                "避免只修表面",
                WorkflowSuggestionKind::BugScan,
            ),
        ],
        WorkflowStage::DocSync => vec![
            suggestion(
                "同步 TODO.md",
                1,
                "保持真实优先级",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "同步 STATUS.md / PROGRESS.md",
                2,
                "保持阶段状态与能力描述准确",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "记录执行日志",
                3,
                "为下一轮提供上下文",
                WorkflowSuggestionKind::DocSync,
            ),
        ],
        WorkflowStage::CommitPush => vec![
            suggestion(
                "commit 当前稳定成果",
                1,
                "把本轮成果落盘，便于继续迭代",
                WorkflowSuggestionKind::Refactor,
            ),
            suggestion(
                "整理本地验证记录",
                2,
                "确保下一轮仍以本地结果为准",
                WorkflowSuggestionKind::DocSync,
            ),
            suggestion(
                "准备下一轮 focus",
                3,
                "让循环衔接更顺滑",
                WorkflowSuggestionKind::DocSync,
            ),
        ],
        WorkflowStage::Cooldown => vec![
            suggestion(
                "短暂冷却并等待下一轮",
                1,
                "避免高频抖动与误判",
                WorkflowSuggestionKind::Performance,
            ),
            suggestion(
                "检查是否要回到 plan",
                2,
                "保持循环节奏",
                WorkflowSuggestionKind::Refactor,
            ),
        ],
        WorkflowStage::Blocked => vec![
            suggestion(
                "识别阻塞原因",
                1,
                "先明确为什么不能继续",
                WorkflowSuggestionKind::BugScan,
            ),
            suggestion(
                "给出恢复路径",
                2,
                "为人工接管或下一轮恢复做准备",
                WorkflowSuggestionKind::DocSync,
            ),
        ],
    }
}

fn suggestion(
    title: &str,
    priority: u8,
    rationale: &str,
    kind: WorkflowSuggestionKind,
) -> WorkflowSuggestion {
    WorkflowSuggestion {
        title: title.to_string(),
        priority,
        rationale: rationale.to_string(),
        kind,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn workflow_state_roundtrip_works() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("RUN_STATE.json");
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        state.loop_enabled = true;
        state.next_suggestions = vec![WorkflowSuggestion {
            title: "实现工作流状态机骨架".to_string(),
            priority: 1,
            rationale: "这是自动循环执行器的起点".to_string(),
            kind: WorkflowSuggestionKind::Feature,
        }];
        state.save(&path).expect("save state");
        let loaded = WorkflowExecutionState::load(&path).expect("load state");
        assert_eq!(loaded, state);
    }

    #[test]
    fn workflow_state_enters_bug_cycle_after_threshold() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        state.completed_since_bug_cycle = 3;
        assert!(state.should_enter_bug_cycle());
        state.advance_after_success();
        assert_eq!(state.stage, WorkflowStage::BugScan);
        assert_eq!(
            state.next_suggestions[0].kind,
            WorkflowSuggestionKind::BugScan
        );
    }

    #[test]
    fn workflow_state_failure_redirects_to_bug_scan() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        state.stage = WorkflowStage::Execute;
        state.mark_failure("integration test failed");
        assert_eq!(state.stage, WorkflowStage::BugScan);
        assert_eq!(state.consecutive_failures, 1);
        assert_eq!(state.last_result_summary, "integration test failed");
        assert_eq!(state.next_suggestions[0].title, "查找 bug");
    }

    #[test]
    fn workflow_state_enters_blocked_after_too_many_failures() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        state.mark_failure("fail-1");
        state.mark_failure("fail-2");
        state.mark_failure("fail-3");
        assert_eq!(state.stage, WorkflowStage::Blocked);
        assert!(state.blocked_reason.contains("连续失败次数过高"));
    }

    #[test]
    fn workflow_state_can_migrate_legacy_run_state() {
        let legacy = r#"{
          "project": "PersonaPilot",
          "currentRound": 78,
          "roundType": "plan",
          "currentObjective": "进入 build 轮，新增具体 SQLite schema 设计文档。",
          "lastVerificationResult": "已完成新一轮 plan，锁定下一步为细化 SQLite schema 草案。",
          "failureCount": 2,
          "lastSchedulerDecision": "Executed plan, next=build",
          "nextRoundType": "build",
          "schedulerStatus": "running",
          "currentFocus": "周期执行协议已落地",
          "nextRecommendedActions": ["基于状态机进行 1 个 mini-cycle 试运行", "初始化 Cargo 工程"]
        }"#;
        let state = WorkflowExecutionState::from_json_str(legacy).expect("migrate legacy state");
        assert_eq!(state.loop_iteration, 78);
        assert_eq!(state.stage, WorkflowStage::Execute);
        assert_eq!(state.consecutive_failures, 2);
        assert_eq!(state.next_suggestions.len(), 2);
        assert!(state.loop_enabled);
    }

    #[test]
    fn ensure_default_state_file_creates_new_state() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("RUN_STATE.json");
        let state = WorkflowExecutionState::ensure_default_state_file(&path, "PersonaPilot")
            .expect("ensure state");
        assert_eq!(state.project, "PersonaPilot");
        assert!(path.exists());
        assert!(!state.next_suggestions.is_empty());
    }

    #[test]
    fn default_plan_suggestions_are_ranked_and_complete() {
        let suggestions = default_suggestions_for_stage(WorkflowStage::Plan);
        assert_eq!(suggestions.len(), 5);
        assert_eq!(suggestions[0].priority, 1);
        assert_eq!(suggestions[0].kind, WorkflowSuggestionKind::DocSync);
        assert!(suggestions.iter().all(|s| !s.rationale.is_empty()));
    }

    #[test]
    fn minimal_cycle_step_advances_plan_execute_verify() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        assert_eq!(state.stage, WorkflowStage::Plan);
        run_minimal_cycle_step(&mut state);
        assert_eq!(state.stage, WorkflowStage::Execute);
        run_minimal_cycle_step(&mut state);
        assert_eq!(state.stage, WorkflowStage::Verify);
        assert_eq!(state.last_executed_actions.len(), 2);
        run_minimal_cycle_step(&mut state);
        assert_eq!(state.stage, WorkflowStage::BugScan);
        assert!(state.last_result_summary.contains("verify"));
    }

    #[test]
    fn cooldown_stage_uses_configured_delay_in_summary() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        state.stage = WorkflowStage::Cooldown;
        state.cooldown_seconds = 45;
        run_minimal_cycle_step(&mut state);
        assert!(state.last_result_summary.contains("45 秒"));
    }

    #[test]
    fn dispatch_top_suggestions_executes_first_two_items() {
        let mut state = WorkflowExecutionState::new("PersonaPilot");
        let result = dispatch_top_suggestions(&mut state, 2);
        assert_eq!(result.executed.len(), 2);
        assert_eq!(state.last_executed_actions.len(), 2);
        assert!(state
            .last_executed_actions
            .iter()
            .all(|a| a.status == "logged" || a.status == "log_failed"));
    }

    #[test]
    fn tick_workflow_file_updates_run_state_on_disk() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("RUN_STATE.json");
        let state = tick_workflow_file(&path, "PersonaPilot").expect("tick workflow file");
        assert_eq!(state.stage, WorkflowStage::Execute);
        let loaded = WorkflowExecutionState::load(&path).expect("load saved workflow state");
        assert_eq!(loaded.stage, WorkflowStage::Execute);
    }

    #[test]
    fn run_minimal_cycle_steps_persists_multiple_stage_advances() {
        let dir = tempdir().expect("tempdir");
        let path = dir.path().join("RUN_STATE.json");
        let state = run_minimal_cycle_steps(&path, "PersonaPilot", 3).expect("run workflow steps");
        assert_eq!(state.stage, WorkflowStage::BugScan);
        let loaded = WorkflowExecutionState::load(&path).expect("load saved workflow state");
        assert_eq!(loaded.stage, WorkflowStage::BugScan);
    }

    #[test]
    fn dynamic_suggestions_follow_current_project_direction() {
        let ctx = WorkflowDocumentContext {
            vision: "artifact 可替换执行引擎".to_string(),
            current_direction: "trust score verify 文档 写放大".to_string(),
            todo: "同步 CURRENT_* / TODO / STATUS 口径，压平旧阶段残留".to_string(),
        };
        let suggestions = generate_dynamic_suggestions(WorkflowStage::Plan, &ctx);
        assert!(!suggestions.is_empty());
        assert!(suggestions.iter().any(|s| s.title.contains("trust score")));
        assert!(suggestions
            .iter()
            .any(|s| s.kind == WorkflowSuggestionKind::DocSync));
    }
}
