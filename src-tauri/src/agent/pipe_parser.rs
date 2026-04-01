use crate::agent::codex_parser::extract_codex_delta;
use crate::models::BusEvent;
use serde_json::Value;

/// Trait for parsing structured stdout in pipe-exec mode.
/// NOT a general protocol parser — session_actor has its own protocol handling.
/// Implementations parse agent-specific NDJSON into normalized BusEvents.
pub trait PipeStdoutParser: Send {
    /// Parse one NDJSON line into zero or more BusEvents.
    fn parse_line(&mut self, run_id: &str, raw: &Value) -> Vec<BusEvent>;
}

/// Codex NDJSON parser.
/// Current coverage: text delta extraction (agent_message, command_execution).
/// Phase 2 will add item.started, turn.failed, error, etc.
pub struct CodexStdoutParser;

impl PipeStdoutParser for CodexStdoutParser {
    fn parse_line(&mut self, run_id: &str, raw: &Value) -> Vec<BusEvent> {
        if let Some(text) = extract_codex_delta(raw) {
            vec![BusEvent::MessageDelta {
                run_id: run_id.to_string(),
                text,
                parent_tool_use_id: None,
            }]
        } else {
            vec![]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn codex_agent_message_returns_delta() {
        let mut parser = CodexStdoutParser;
        let raw =
            json!({"type": "item.completed", "item": {"type": "agent_message", "text": "Hello"}});
        let events = parser.parse_line("run-1", &raw);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BusEvent::MessageDelta { text, .. } => assert_eq!(text, "Hello"),
            other => panic!("expected MessageDelta, got {:?}", other),
        }
    }

    #[test]
    fn codex_command_execution_returns_formatted_delta() {
        let mut parser = CodexStdoutParser;
        let raw = json!({"type": "item.completed", "item": {"type": "command_execution", "command": "ls", "output": "a.rs\nb.rs"}});
        let events = parser.parse_line("run-1", &raw);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BusEvent::MessageDelta { text, .. } => assert!(text.contains("$ ls")),
            other => panic!("expected MessageDelta, got {:?}", other),
        }
    }

    #[test]
    fn codex_thread_started_returns_empty() {
        // thread.started is NOT a text delta — parser correctly returns empty
        // (conversation_ref capture happens in stream.rs, not here)
        let mut parser = CodexStdoutParser;
        let raw = json!({"type": "thread.started", "thread_id": "t1"});
        let events = parser.parse_line("run-1", &raw);
        assert!(events.is_empty());
    }

    #[test]
    fn codex_turn_failed_returns_empty() {
        // turn.failed not yet handled — returns empty (Phase 2 will add BusEvent mapping)
        let mut parser = CodexStdoutParser;
        let raw = json!({"type": "turn.failed", "error": {"message": "timeout"}});
        let events = parser.parse_line("run-1", &raw);
        assert!(events.is_empty());
    }
}
