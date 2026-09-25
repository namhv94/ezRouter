// src/context_optimizer.rs
// Conservative context compaction for long Codex requests.
// Feature-flagged OFF by default. Fail-closed: any error returns messages unchanged.

use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::provider::{ChatCompletionRequest, ChatMessage, MessageContent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOptimizerConfig {
    pub enabled: bool,
    pub max_messages: usize,      // 0 = disabled
    pub max_bytes: usize,         // 0 = disabled
    pub recent_turns_keep: usize, // always keep this many recent user/assistant turns
}

impl Default for ContextOptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_messages: 0,
            max_bytes: 0,
            recent_turns_keep: 20,
        }
    }
}

/// Metrics về context trước và sau khi compact (logging only, không log nội dung)
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct ContextMetrics {
    pub total_messages: usize,
    /// Total estimated bytes across all messages BEFORE compaction (pre-compaction).
    pub total_bytes: usize,
    pub system_messages: usize,
    pub tool_call_messages: usize,   // assistant with tool_calls
    pub tool_result_messages: usize, // role=tool
    pub user_messages: usize,
    pub assistant_messages: usize,
    /// Count of modified tool-result messages compacted (0 when disabled, fail-closed, or nothing compacted).
    pub estimated_compacted: usize,
    /// Total estimated bytes saved post-compaction.
    pub compacted_bytes: usize,
}

/// Estimate memory/text bytes of a single ChatMessage without cloning.
fn estimate_message_bytes(msg: &ChatMessage) -> usize {
    let mut bytes = msg.role.len();
    if let Some(ref name) = msg.name {
        bytes += name.len();
    }
    if let Some(ref tool_call_id) = msg.tool_call_id {
        bytes += tool_call_id.len();
    }
    if let Some(ref content) = msg.content {
        match content {
            MessageContent::Text(s) => bytes += s.len(),
            MessageContent::Parts(parts) => {
                for p in parts {
                    if let Some(s) = p.as_str() {
                        bytes += s.len();
                    } else if let Some(s) = p.get("text").and_then(|t| t.as_str()) {
                        bytes += s.len();
                    } else if let Some(s) = p.get("input_text").and_then(|t| t.as_str()) {
                        bytes += s.len();
                    } else if let Some(s) = p.get("content").and_then(|t| t.as_str()) {
                        bytes += s.len();
                    } else {
                        bytes += p.to_string().len();
                    }
                }
            }
        }
    }
    if let Some(ref tool_calls) = msg.tool_calls {
        bytes += tool_calls.to_string().len();
    }
    bytes
}

/// Measure context metrics directly from a slice of ChatMessages without cloning.
pub fn measure_context_slice(messages: &[ChatMessage]) -> ContextMetrics {
    let mut metrics = ContextMetrics {
        total_messages: messages.len(),
        ..Default::default()
    };

    for msg in messages {
        let msg_bytes = estimate_message_bytes(msg);
        metrics.total_bytes += msg_bytes;

        let role_lower = msg.role.to_ascii_lowercase();
        if role_lower == "system" {
            metrics.system_messages += 1;
        } else if role_lower == "user" {
            metrics.user_messages += 1;
        } else if role_lower == "assistant" {
            metrics.assistant_messages += 1;
            if msg.tool_calls.is_some() {
                metrics.tool_call_messages += 1;
            }
        } else if role_lower == "tool" || role_lower == "function" {
            metrics.tool_result_messages += 1;
        }
    }

    metrics
}

/// Measure context metrics mà không log nội dung
pub fn measure_context(req: &ChatCompletionRequest) -> ContextMetrics {
    measure_context_slice(&req.messages)
}

/// Extract tool call IDs and their function names from an assistant message's tool_calls value.
fn extract_tool_calls(tool_calls_val: &Option<serde_json::Value>) -> Vec<(String, String)> {
    let mut results = Vec::new();
    if let Some(ref val) = tool_calls_val {
        if let Some(arr) = val.as_array() {
            for item in arr {
                let id = item
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let name = item
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("")
                    .to_string();
                if !id.is_empty() {
                    results.push((id, name));
                }
            }
        }
    }
    results
}

/// Apply conservative compaction:
/// - Luôn giữ toàn bộ system messages
/// - Luôn giữ recent_turns_keep lượt user turns gần nhất (mỗi user message = 1 turn)
/// - Luôn giữ assistant tool_calls cùng matching tool result (atomic group)
/// - Chỉ compact historical tool results nằm ngoài recent window, đã đóng
/// - Bảo toàn OOB steering markers ([OUT-OF-BAND USER MESSAGE và [SKILL_PRUNED])
/// - Thay content dài bằng: [Historical tool result compacted: tool=<name>, bytes=<n>, status=ok]
/// - KHÔNG compact current tool continuation, structured JSON response
/// - KHÔNG truncate UTF-8 giữa chừng
/// - KHÔNG sửa call ID, thought signature
/// - KHÔNG xóa user/system messages
///
/// Khi flag disabled hoặc bất kỳ lỗi nào: trả về messages nguyên bản (fail-closed)
pub fn compact_messages(
    messages: Vec<ChatMessage>,
    config: &ContextOptimizerConfig,
) -> (Vec<ChatMessage>, ContextMetrics) {
    if !config.enabled {
        let mut metrics = measure_context_slice(&messages);
        metrics.estimated_compacted = 0;
        metrics.compacted_bytes = 0;
        return (messages, metrics);
    }

    let original = messages.clone();
    let result = catch_unwind(AssertUnwindSafe(|| {
        compact_messages_internal(messages, config)
    }));

    match result {
        Ok(res) => res,
        Err(_) => {
            let mut m = measure_context_slice(&original);
            m.estimated_compacted = 0;
            m.compacted_bytes = 0;
            (original, m)
        }
    }
}

fn compact_messages_internal(
    mut messages: Vec<ChatMessage>,
    config: &ContextOptimizerConfig,
) -> (Vec<ChatMessage>, ContextMetrics) {
    let original = messages.clone();
    let initial_metrics = measure_context_slice(&messages);

    if messages.is_empty() {
        return (messages, initial_metrics);
    }

    let n = messages.len();

    // 1. Identify recent window boundary.
    // Count user messages from the end (each user message = 1 turn).
    // Keep all messages from the recent_turns_keep-th user message to the end.
    let mut turns_seen = 0;
    let mut recent_cutoff_idx = 0; // default: keep all if not enough turns

    if config.recent_turns_keep > 0 {
        for i in (0..n).rev() {
            if messages[i].role.eq_ignore_ascii_case("user") {
                turns_seen += 1;
                if turns_seen >= config.recent_turns_keep {
                    recent_cutoff_idx = i;
                    break;
                }
            }
        }
    } else {
        recent_cutoff_idx = n;
    }

    // 2. Build index of tool calls and matching tool results:
    let mut assistant_tool_calls: Vec<(usize, Vec<(String, String)>)> = Vec::new();
    let mut tool_results_map: HashMap<String, Vec<usize>> = HashMap::new();

    for (idx, msg) in messages.iter().enumerate() {
        let role = msg.role.to_ascii_lowercase();
        if role == "assistant" {
            let calls = extract_tool_calls(&msg.tool_calls);
            if !calls.is_empty() {
                assistant_tool_calls.push((idx, calls));
            }
        } else if (role == "tool" || role == "function") && msg.tool_call_id.is_some() {
            if let Some(ref id) = msg.tool_call_id {
                tool_results_map.entry(id.clone()).or_default().push(idx);
            }
        }
    }

    // 3. Classify tool-call groups:
    const OOB_MARKER: &str = "[OUT-OF-BAND USER MESSAGE";
    const SKILL_PRUNED: &str = "[SKILL_PRUNED]";

    let mut uncompactable_tool_ids: HashSet<String> = HashSet::new();

    #[derive(Debug)]
    struct EligibleGroup {
        assistant_idx: usize,
        tool_result_indices: Vec<usize>,
    }
    let mut eligible_groups: Vec<EligibleGroup> = Vec::new();

    for (asst_idx, calls) in &assistant_tool_calls {
        let mut group_is_open = false;
        let mut group_touches_recent = *asst_idx >= recent_cutoff_idx;
        let mut group_has_protected_marker = false;
        let mut group_tool_indices: Vec<usize> = Vec::new();

        for (call_id, _) in calls {
            match tool_results_map.get(call_id) {
                None => {
                    group_is_open = true;
                    uncompactable_tool_ids.insert(call_id.clone());
                }
                Some(indices) => {
                    for &t_idx in indices {
                        group_tool_indices.push(t_idx);
                        if t_idx >= recent_cutoff_idx {
                            group_touches_recent = true;
                            uncompactable_tool_ids.insert(call_id.clone());
                        }
                    }
                }
            }
        }

        // Check if any tool result in this group contains protected markers (OOB/SKILL_PRUNED)
        for &t_idx in &group_tool_indices {
            let content = messages[t_idx].content_text();
            if content.contains(OOB_MARKER) || content.contains(SKILL_PRUNED) {
                group_has_protected_marker = true;
                break;
            }
        }

        if group_is_open || group_touches_recent {
            for (call_id, _) in calls {
                uncompactable_tool_ids.insert(call_id.clone());
            }
        }

        // An eligible group for pruning must be:
        // - fully closed (all tool calls have tool results)
        // - assistant and all tool results strictly outside recent window
        // - contains no protected markers (OOB / SKILL_PRUNED)
        if !group_is_open && !group_touches_recent && !group_has_protected_marker {
            eligible_groups.push(EligibleGroup {
                assistant_idx: *asst_idx,
                tool_result_indices: group_tool_indices,
            });
        }
    }

    let mut tool_name_map: HashMap<String, String> = HashMap::new();
    for (_, calls) in &assistant_tool_calls {
        for (id, name) in calls {
            tool_name_map.insert(id.clone(), name.clone());
        }
    }

    // 4. Content compaction on historical closed tool results outside the recent window
    let mut tool_results_compacted: HashSet<usize> = HashSet::new();
    for (idx, msg) in messages.iter_mut().enumerate() {
        if idx >= recent_cutoff_idx {
            continue;
        }

        let role = msg.role.to_ascii_lowercase();
        if role == "tool" || role == "function" {
            if let Some(ref id) = msg.tool_call_id {
                if !uncompactable_tool_ids.contains(id) {
                    let content_str = msg.content_text();
                    if content_str.contains(OOB_MARKER) || content_str.contains(SKILL_PRUNED) {
                        continue;
                    }

                    let tool_name = tool_name_map
                        .get(id)
                        .map(|s| s.as_str())
                        .unwrap_or("unknown");
                    let original_bytes = content_str.len();

                    let marker = format!(
                        "[Historical tool result compacted: tool={}, bytes={}, status=ok]",
                        tool_name, original_bytes
                    );
                    if original_bytes > 100 && original_bytes > marker.len() {
                        msg.content = Some(MessageContent::Text(marker));
                        tool_results_compacted.insert(idx);
                    }
                }
            }
        }
    }

    // 5. Enforce max_messages and max_bytes through conservative pruning
    let mut current_count = messages.len();
    let mut current_bytes: usize = messages.iter().map(estimate_message_bytes).sum();

    let need_prune = (config.max_messages > 0 && current_count > config.max_messages)
        || (config.max_bytes > 0 && current_bytes > config.max_bytes);

    let mut indices_to_remove: HashSet<usize> = HashSet::new();

    if need_prune {
        eligible_groups.sort_by_key(|g| g.assistant_idx);

        for group in eligible_groups {
            if (config.max_messages == 0 || current_count <= config.max_messages)
                && (config.max_bytes == 0 || current_bytes <= config.max_bytes)
            {
                break;
            }

            let asst_bytes = estimate_message_bytes(&messages[group.assistant_idx]);
            let tools_bytes: usize = group
                .tool_result_indices
                .iter()
                .map(|&idx| estimate_message_bytes(&messages[idx]))
                .sum();
            let group_bytes = asst_bytes + tools_bytes;
            let group_count = 1 + group.tool_result_indices.len();

            indices_to_remove.insert(group.assistant_idx);
            for &t_idx in &group.tool_result_indices {
                indices_to_remove.insert(t_idx);
            }

            current_count = current_count.saturating_sub(group_count);
            current_bytes = current_bytes.saturating_sub(group_bytes);
        }
    }

    // 6. Fail-closed check: if hard limits cannot be achieved without violating invariants,
    // return original messages unchanged.
    if (config.max_messages > 0 && current_count > config.max_messages)
        || (config.max_bytes > 0 && current_bytes > config.max_bytes)
    {
        let mut m = measure_context_slice(&original);
        m.estimated_compacted = 0;
        m.compacted_bytes = 0;
        return (original, m);
    }

    // 7. Materialize final message list
    let pruned_count = indices_to_remove.len();
    let final_messages: Vec<ChatMessage> = messages
        .into_iter()
        .enumerate()
        .filter(|(idx, _)| !indices_to_remove.contains(idx))
        .map(|(_, msg)| msg)
        .collect();

    let retained_compacted = tool_results_compacted
        .iter()
        .filter(|idx| !indices_to_remove.contains(idx))
        .count();

    let final_bytes: usize = final_messages.iter().map(estimate_message_bytes).sum();

    let mut metrics = initial_metrics;
    metrics.estimated_compacted = retained_compacted + pruned_count;
    metrics.compacted_bytes = metrics.total_bytes.saturating_sub(final_bytes);

    (final_messages, metrics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_disabled_noop() {
        let config = ContextOptimizerConfig {
            enabled: false,
            ..Default::default()
        };
        let messages = vec![
            ChatMessage::system("You are a helpful assistant."),
            ChatMessage::user("Hello!"),
            ChatMessage::assistant(Some("Hi there!"), None),
        ];

        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
        assert_eq!(metrics.total_messages, 3);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_keeps_system_messages() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1,
            ..Default::default()
        };
        let system_text = "System instruction ".repeat(50);
        let messages = vec![
            ChatMessage::system(system_text.as_str()),
            ChatMessage::user("Old turn 1"),
            ChatMessage::assistant(Some("Old response 1"), None),
            ChatMessage::user("Recent turn"),
            ChatMessage::assistant(Some("Recent response"), None),
        ];

        let (compacted, _metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted[0].role, "system");
        assert_eq!(compacted[0].content_text(), system_text);
    }

    #[test]
    fn test_optimizer_keeps_recent_turns() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 2, // Keep last 2 user turns
            ..Default::default()
        };

        // 3 user turns: each has user + assistant + tool result
        // Turn 1 (historical): should have tool result compacted
        // Turn 2 & Turn 3 (recent): should have tool results preserved intact
        let messages = vec![
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("Calling tool 1"),
                Some(serde_json::json!([
                    {"id": "call_1", "function": {"name": "read_file"}}
                ])),
            ),
            ChatMessage::tool("call_1", "old turn 1 tool content ".repeat(30)),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(
                Some("Calling tool 2"),
                Some(serde_json::json!([
                    {"id": "call_2", "function": {"name": "search"}}
                ])),
            ),
            ChatMessage::tool("call_2", "recent turn 2 tool result ".repeat(30)),
            ChatMessage::user("Turn 3"),
            ChatMessage::assistant(
                Some("Calling tool 3"),
                Some(serde_json::json!([
                    {"id": "call_3", "function": {"name": "bash"}}
                ])),
            ),
            ChatMessage::tool("call_3", "recent turn 3 tool result ".repeat(30)),
        ];

        let (compacted, _metrics) = compact_messages(messages, &config);
        // Turn 1 tool result compacted
        assert!(compacted[2]
            .content_text()
            .contains("Historical tool result compacted"));
        // Turn 2 and Turn 3 tool results kept intact because user turns = 2
        assert!(compacted[5]
            .content_text()
            .contains("recent turn 2 tool result"));
        assert!(compacted[8]
            .content_text()
            .contains("recent turn 3 tool result"));
    }

    #[test]
    fn test_optimizer_oob_steering_preserved() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1, // Only keep Turn 2
            ..Default::default()
        };

        let normal_tool_content = "large output from command ".repeat(20);
        let oob_tool_content = format!(
            "tool output before\n[OUT-OF-BAND USER MESSAGE — direct user steering]\nstop that!\n[/OUT-OF-BAND USER MESSAGE]\n{}",
            "more tool output ".repeat(15)
        );
        let skill_pruned_content = format!(
            "Some tool output with [SKILL_PRUNED] warning placeholder: {}",
            "data ".repeat(30)
        );

        let messages = vec![
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("Call tool 1"),
                Some(serde_json::json!([
                    {"id": "call_normal", "function": {"name": "bash"}},
                    {"id": "call_oob", "function": {"name": "bash"}},
                    {"id": "call_pruned", "function": {"name": "bash"}},
                ])),
            ),
            ChatMessage::tool("call_normal", normal_tool_content.as_str()),
            ChatMessage::tool("call_oob", oob_tool_content.as_str()),
            ChatMessage::tool("call_pruned", skill_pruned_content.as_str()),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("Continuing"), None),
        ];

        let (compacted, _metrics) = compact_messages(messages, &config);

        // Normal tool result outside recent window should be compacted
        assert!(compacted[2]
            .content_text()
            .contains("Historical tool result compacted"));
        // Tool result containing OOB marker must NOT be compacted
        assert_eq!(compacted[3].content_text(), oob_tool_content);
        // Tool result containing [SKILL_PRUNED] must NOT be compacted
        assert_eq!(compacted[4].content_text(), skill_pruned_content);
    }

    #[test]
    fn test_optimizer_max_messages_enforced() {
        let make_messages = || {
            vec![
                ChatMessage::system("system instruction"),
                ChatMessage::user("Turn 1"),
                ChatMessage::assistant(
                    Some("Calling tool 1"),
                    Some(serde_json::json!([
                        {"id": "call_1", "function": {"name": "read_file"}}
                    ])),
                ),
                ChatMessage::tool("call_1", "old tool content ".repeat(30)),
                ChatMessage::user("Turn 2"),
                ChatMessage::assistant(
                    Some("Calling tool 2"),
                    Some(serde_json::json!([
                        {"id": "call_2", "function": {"name": "search"}}
                    ])),
                ),
                ChatMessage::tool("call_2", "recent tool content ".repeat(30)),
                ChatMessage::user("Turn 3"),
                ChatMessage::assistant(Some("Turn 3 response"), None),
            ]
        };

        // Without max_messages limit (max_messages = 0), all turns are kept when recent_turns_keep: 10
        let config_without_max = ContextOptimizerConfig {
            enabled: true,
            max_messages: 0,
            recent_turns_keep: 10,
            ..Default::default()
        };
        let (uncompacted, _) = compact_messages(make_messages(), &config_without_max);
        assert_eq!(uncompacted.len(), 9);
        assert!(uncompacted[3].content_text().contains("old tool content"));

        // With max_messages = 7 and recent_turns_keep: 2:
        // Turn 1 tool call group (asst + tool) is outside recent window and safely pruned.
        // Result length is 7 <= 7, Turn 2 atomic group and all user/system messages preserved.
        let config_with_max = ContextOptimizerConfig {
            enabled: true,
            max_messages: 7,
            recent_turns_keep: 2,
            ..Default::default()
        };
        let (compacted, metrics) = compact_messages(make_messages(), &config_with_max);
        assert_eq!(compacted.len(), 7);
        assert!(compacted.len() <= config_with_max.max_messages);
        assert_eq!(compacted[0].role, "system");
        assert_eq!(compacted[1].content_text(), "Turn 1");
        assert_eq!(compacted[2].content_text(), "Turn 2");
        assert_eq!(compacted[3].role, "assistant");
        assert_eq!(compacted[4].role, "tool");
        assert!(compacted[4].content_text().contains("recent tool content"));
        assert_eq!(compacted[5].content_text(), "Turn 3");
        assert_eq!(compacted[6].content_text(), "Turn 3 response");
        assert_eq!(metrics.estimated_compacted, 2); // 2 messages pruned
        assert!(metrics.compacted_bytes > 0);

        // When hard limit cannot be achieved without violating invariants (e.g. max_messages = 5
        // with recent_turns_keep: 2, leaving 7 messages > 5): fail-closed and return original.
        let config_impossible = ContextOptimizerConfig {
            enabled: true,
            max_messages: 5,
            recent_turns_keep: 2,
            ..Default::default()
        };
        let (fallback, fallback_metrics) = compact_messages(make_messages(), &config_impossible);
        assert_eq!(fallback.len(), 9);
        assert_eq!(fallback_metrics.estimated_compacted, 0);
        assert_eq!(fallback_metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_max_bytes_enforced() {
        let make_messages = || {
            vec![
                ChatMessage::system("system instruction"),
                ChatMessage::user("Turn 1"),
                ChatMessage::assistant(
                    Some("Calling tool 1"),
                    Some(serde_json::json!([
                        {"id": "call_1", "function": {"name": "read_file"}}
                    ])),
                ),
                ChatMessage::tool("call_1", "old tool content ".repeat(30)),
                ChatMessage::user("Turn 2"),
                ChatMessage::assistant(
                    Some("Calling tool 2"),
                    Some(serde_json::json!([
                        {"id": "call_2", "function": {"name": "search"}}
                    ])),
                ),
                ChatMessage::tool("call_2", "recent tool content ".repeat(30)),
                ChatMessage::user("Turn 3"),
                ChatMessage::assistant(Some("Turn 3 response"), None),
            ]
        };

        // Pre-compaction bytes is ~1300.
        // After Turn 1 tool result compaction, bytes is ~863.
        // With recent_turns_keep: 2 and max_bytes: 800, tool result compaction alone
        // (~863 bytes) is insufficient, so Call 1 atomic group is pruned, reducing total bytes to <= 800.
        let config_with_max_bytes = ContextOptimizerConfig {
            enabled: true,
            max_bytes: 800,
            recent_turns_keep: 2,
            ..Default::default()
        };
        let (compacted, metrics) = compact_messages(make_messages(), &config_with_max_bytes);
        let actual_bytes: usize = compacted.iter().map(estimate_message_bytes).sum();
        assert!(actual_bytes <= config_with_max_bytes.max_bytes);
        assert_eq!(compacted.len(), 7); // Call 1 group pruned
        assert!(metrics.compacted_bytes > 0);

        // Fail-closed when max_bytes is impossibly small (e.g. smaller than system message):
        let config_impossible = ContextOptimizerConfig {
            enabled: true,
            max_bytes: 10,
            recent_turns_keep: 2,
            ..Default::default()
        };
        let (fallback, fallback_metrics) = compact_messages(make_messages(), &config_impossible);
        assert_eq!(fallback.len(), 9);
        assert_eq!(fallback_metrics.estimated_compacted, 0);
        assert_eq!(fallback_metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_pruning_multi_tool_call_atomic_group() {
        let config = ContextOptimizerConfig {
            enabled: true,
            max_messages: 5,
            recent_turns_keep: 1, // Only keep Turn 2
            ..Default::default()
        };

        // Turn 1 has 1 assistant with 2 parallel tool calls (call_a and call_b) and 2 tool results.
        // If pruned, all 3 messages (assistant + tool_a + tool_b) must be removed together!
        let messages = vec![
            ChatMessage::system("system instruction"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("Calling two tools"),
                Some(serde_json::json!([
                    {"id": "call_a", "function": {"name": "read_file"}},
                    {"id": "call_b", "function": {"name": "grep_file"}}
                ])),
            ),
            ChatMessage::tool("call_a", "content a ".repeat(20)),
            ChatMessage::tool("call_b", "content b ".repeat(20)),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("Turn 2 reply"), None),
        ];

        let (compacted, metrics) = compact_messages(messages, &config);
        // Original has 7 messages. Pruning Turn 1 tool group removes 3 messages -> 4 messages remain <= 5.
        assert_eq!(compacted.len(), 4);
        assert_eq!(compacted[0].role, "system");
        assert_eq!(compacted[1].content_text(), "Turn 1");
        assert_eq!(compacted[2].content_text(), "Turn 2");
        assert_eq!(compacted[3].content_text(), "Turn 2 reply");
        // Neither tool_a nor tool_b was left behind (atomic group preserved)
        for msg in &compacted {
            assert_ne!(msg.role, "tool");
        }
        assert_eq!(metrics.estimated_compacted, 3); // 3 messages pruned
    }

    #[test]
    fn test_optimizer_pruning_never_removes_open_tool_calls() {
        let config = ContextOptimizerConfig {
            enabled: true,
            max_messages: 3,
            recent_turns_keep: 1,
            ..Default::default()
        };

        // Turn 1 has an OPEN tool call (no tool result exists).
        let messages = vec![
            ChatMessage::system("system instruction"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("Calling open tool"),
                Some(serde_json::json!([
                    {"id": "call_open", "function": {"name": "bash"}}
                ])),
            ),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("Continuing"), None),
        ];

        // 5 messages total. max_messages = 3.
        // Open tool call cannot be removed. System and user messages cannot be removed.
        // Limit cannot be achieved without violating invariants -> must fail-closed.
        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_pruning_preserves_oob_and_skill_pruned() {
        let config = ContextOptimizerConfig {
            enabled: true,
            max_messages: 5,
            recent_turns_keep: 1,
            ..Default::default()
        };

        let messages = vec![
            ChatMessage::system("system instruction"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("Calling tool"),
                Some(serde_json::json!([
                    {"id": "call_protected", "function": {"name": "bash"}}
                ])),
            ),
            ChatMessage::tool(
                "call_protected",
                "output with [OUT-OF-BAND USER MESSAGE — steering] keep safe [/OUT-OF-BAND USER MESSAGE]",
            ),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("Done"), None),
        ];

        // 6 messages total. max_messages = 5.
        // Turn 1 contains OOB marker, so it CANNOT be pruned.
        // Limit cannot be achieved without violating invariants -> must fail-closed.
        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_pruning_never_removes_user_or_system() {
        let config = ContextOptimizerConfig {
            enabled: true,
            max_messages: 4,
            recent_turns_keep: 1,
            ..Default::default()
        };

        // 1 system message + 4 user messages = 5 messages. No tool calls.
        let messages = vec![
            ChatMessage::system("system 1"),
            ChatMessage::user("user 1"),
            ChatMessage::user("user 2"),
            ChatMessage::user("user 3"),
            ChatMessage::user("user 4"),
        ];

        // Cannot prune user or system messages to satisfy max_messages = 4.
        // Must fail-closed.
        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_compacts_old_tool_results() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1, // Only keep the last turn (user Turn 2 + assistant Turn 2)
            ..Default::default()
        };

        let old_content = "large file content ".repeat(20);
        let messages = vec![
            ChatMessage::system("system prompt"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("call read_file"),
                Some(serde_json::json!([
                    {"id": "call_old", "function": {"name": "read_file"}}
                ])),
            ),
            ChatMessage::tool("call_old", old_content.as_str()),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("done"), None),
        ];

        let (compacted, metrics) = compact_messages(messages, &config);
        let tool_msg = &compacted[3];
        assert_eq!(tool_msg.role, "tool");
        assert_eq!(tool_msg.tool_call_id.as_deref(), Some("call_old"));
        assert!(tool_msg
            .content_text()
            .contains("[Historical tool result compacted: tool=read_file, bytes="));
        assert!(tool_msg.content_text().contains("status=ok]"));
        assert_eq!(metrics.estimated_compacted, 1);
        assert!(metrics.compacted_bytes > 0);
    }

    #[test]
    fn test_optimizer_keeps_open_tool_calls() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1,
            ..Default::default()
        };

        // Open tool call: assistant has tool_calls, but no tool result exists
        let messages = vec![
            ChatMessage::user("Run something"),
            ChatMessage::assistant(
                Some("calling"),
                Some(serde_json::json!([
                    {"id": "call_open", "function": {"name": "bash"}}
                ])),
            ),
        ];

        let (compacted, _metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
    }

    #[test]
    fn test_optimizer_atomic_tool_group() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 2,
            ..Default::default()
        };

        let messages = vec![
            ChatMessage::user("Old turn"),
            ChatMessage::assistant(Some("Old reply"), None),
            // Atomic group within recent turns
            ChatMessage::user("Turn with tool"),
            ChatMessage::assistant(
                Some("Invoking tool"),
                Some(serde_json::json!([
                    {"id": "call_atomic", "function": {"name": "calc"}}
                ])),
            ),
            ChatMessage::tool("call_atomic", "calc result ".repeat(20)),
            ChatMessage::assistant(Some("Result is 42"), None),
        ];

        let (compacted, _metrics) = compact_messages(messages.clone(), &config);
        // Matching tool result for call_atomic must not be separated or corrupted
        assert_eq!(compacted[3].tool_calls, messages[3].tool_calls);
        assert_eq!(compacted[4].tool_call_id, messages[4].tool_call_id);
        assert_eq!(compacted[4].content_text(), messages[4].content_text());
    }

    #[test]
    fn test_optimizer_utf8_safe() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1,
            ..Default::default()
        };

        let unicode_content = "Xin chào các bạn 🦀! Tiếng Việt có dấu và emoji 🎉. ".repeat(20);
        let messages = vec![
            ChatMessage::user("Start"),
            ChatMessage::assistant(
                Some("Calling tool"),
                Some(serde_json::json!([
                    {"id": "call_utf8", "function": {"name": "vietnamese_echo"}}
                ])),
            ),
            ChatMessage::tool("call_utf8", unicode_content.as_str()),
            ChatMessage::user("End turn"),
            ChatMessage::assistant(Some("Completed"), None),
        ];

        let (compacted, _metrics) = compact_messages(messages, &config);
        let compacted_str = compacted[2].content_text();
        // Valid UTF-8 string guaranteed (Rust std::string)
        assert!(std::str::from_utf8(compacted_str.as_bytes()).is_ok());
        assert!(compacted_str.contains("Historical tool result compacted"));
    }

    #[test]
    fn test_optimizer_fail_closed() {
        // Test that fail-closed returns original messages
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1,
            ..Default::default()
        };

        let messages = vec![
            ChatMessage::system("system"),
            ChatMessage::user("user message"),
        ];

        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted, messages);
        assert_eq!(metrics.total_messages, 2);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_measure_context_metrics() {
        let messages = vec![
            ChatMessage::system("sys 1"),
            ChatMessage::system("sys 2"),
            ChatMessage::user("user 1"),
            ChatMessage::assistant(
                Some("asst 1"),
                Some(serde_json::json!([{"id": "c1", "function": {"name": "f1"}}])),
            ),
            ChatMessage::tool("c1", "result 1"),
            ChatMessage::user("user 2"),
            ChatMessage::assistant(Some("asst 2"), None),
            ChatMessage::user("user 3"),
            ChatMessage::assistant(Some("asst 3"), None),
            ChatMessage::user("user 4"),
        ];

        let req = ChatCompletionRequest {
            messages,
            ..Default::default()
        };

        let metrics = measure_context(&req);
        assert_eq!(metrics.total_messages, 10);
        assert_eq!(metrics.system_messages, 2);
        assert_eq!(metrics.user_messages, 4);
        assert_eq!(metrics.assistant_messages, 3);
        assert_eq!(metrics.tool_call_messages, 1);
        assert_eq!(metrics.tool_result_messages, 1);
        assert!(metrics.total_bytes > 0);
    }

    #[test]
    fn test_optimizer_300_messages() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 10,
            ..Default::default()
        };

        let mut messages = Vec::with_capacity(300);
        messages.push(ChatMessage::system("system instruction"));

        for i in 1..=149 {
            let call_id = format!("call_{i}");
            messages.push(ChatMessage::user(format!("User query {i}")));
            messages.push(ChatMessage::assistant(
                Some(format!("Assistant response {i}")),
                Some(serde_json::json!([
                    {"id": call_id, "function": {"name": "search"}}
                ])),
            ));
            messages.push(ChatMessage::tool(
                &call_id,
                "search result content ".repeat(20),
            ));
        }

        assert!(messages.len() >= 300);

        let (compacted, metrics) = compact_messages(messages, &config);
        assert_eq!(compacted.len(), metrics.total_messages);
        assert_eq!(compacted[0].role, "system");
        assert_eq!(metrics.estimated_compacted, 139);
        assert!(metrics.compacted_bytes > 0);

        // Old tool results should be compacted
        let first_tool_result = &compacted[3];
        assert_eq!(first_tool_result.role, "tool");
        assert!(first_tool_result
            .content_text()
            .contains("Historical tool result compacted"));

        // Recent tool results should be untouched
        let last_tool_result = compacted.last().unwrap();
        assert_eq!(last_tool_result.role, "tool");
        assert!(last_tool_result
            .content_text()
            .contains("search result content"));
    }

    #[test]
    fn test_optimizer_zero_compaction_short_content() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1,
            ..Default::default()
        };

        // Tool result content <= 100 bytes must NOT be compacted
        let short_content = "short result that is definitely <= 100 bytes";
        assert!(short_content.len() <= 100);

        let messages = vec![
            ChatMessage::system("system prompt"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("call read_file"),
                Some(serde_json::json!([
                    {"id": "call_short", "function": {"name": "read_file"}}
                ])),
            ),
            ChatMessage::tool("call_short", short_content),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("done"), None),
        ];

        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        assert_eq!(compacted[3].content_text(), short_content);
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_zero_compaction_recent_only() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 2,
            ..Default::default()
        };

        let messages = vec![
            ChatMessage::system("system prompt"),
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("call read_file"),
                Some(serde_json::json!([
                    {"id": "call_1", "function": {"name": "read_file"}}
                ])),
            ),
            ChatMessage::tool("call_1", "large tool content ".repeat(20)),
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(Some("done"), None),
        ];

        let (compacted, metrics) = compact_messages(messages.clone(), &config);
        // All user turns are inside recent window, so 0 messages compacted
        assert_eq!(compacted[3].content_text(), messages[3].content_text());
        assert_eq!(metrics.estimated_compacted, 0);
        assert_eq!(metrics.compacted_bytes, 0);
    }

    #[test]
    fn test_optimizer_compact_count_and_bytes_saved() {
        let config = ContextOptimizerConfig {
            enabled: true,
            recent_turns_keep: 1, // Keep only Turn 3
            ..Default::default()
        };

        let messages = vec![
            ChatMessage::system("system prompt"),
            // Turn 1: historical, large tool result -> should compact
            ChatMessage::user("Turn 1"),
            ChatMessage::assistant(
                Some("call tool 1"),
                Some(serde_json::json!([
                    {"id": "call_1", "function": {"name": "tool_one"}}
                ])),
            ),
            ChatMessage::tool("call_1", "output from tool one ".repeat(20)),
            // Turn 2: historical, large tool result -> should compact
            ChatMessage::user("Turn 2"),
            ChatMessage::assistant(
                Some("call tool 2"),
                Some(serde_json::json!([
                    {"id": "call_2", "function": {"name": "tool_two"}}
                ])),
            ),
            ChatMessage::tool("call_2", "output from tool two ".repeat(20)),
            // Turn 3: recent turn -> keep intact
            ChatMessage::user("Turn 3"),
            ChatMessage::assistant(
                Some("call tool 3"),
                Some(serde_json::json!([
                    {"id": "call_3", "function": {"name": "tool_three"}}
                ])),
            ),
            ChatMessage::tool("call_3", "output from tool three ".repeat(20)),
        ];

        let (compacted, metrics) = compact_messages(messages, &config);
        assert_eq!(metrics.estimated_compacted, 2);
        assert!(metrics.compacted_bytes > 0);
        assert!(metrics.total_bytes > metrics.compacted_bytes);

        assert!(compacted[3]
            .content_text()
            .contains("Historical tool result compacted: tool=tool_one"));
        assert!(compacted[6]
            .content_text()
            .contains("Historical tool result compacted: tool=tool_two"));
        assert!(compacted[9]
            .content_text()
            .contains("output from tool three"));
    }
}
