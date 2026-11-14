//! Tool calling for models without native tool support (`tool_mode: json`).
//!
//! Tools are described in the system prompt, and the model must answer with
//! one JSON object: `{"tool": "<name>", "arguments": {...}}` to call a tool,
//! or `{"final": "<answer>"}` to finish. Replies are validated against the
//! tool list and each tool's required arguments.

use serde_json::{Value, json};

use crate::types::{
    CompletionRequest, CompletionResponse, Message, Part, Role, StopReason, ToolCall, ToolSpec,
};

/// Rewrite a request with tools into a plain-text request for JSON mode.
/// Requests without tools are returned unchanged.
pub fn to_json_mode(req: &CompletionRequest) -> CompletionRequest {
    if req.tools.is_empty() {
        return req.clone();
    }
    let tools: Vec<Value> = req
        .tools
        .iter()
        .map(|t| json!({"name": t.name, "description": t.description, "arguments_schema": t.input_schema}))
        .collect();
    let protocol = format!(
        "You can call tools. Tools:\n{}\n\n\
         Reply with exactly one JSON object and nothing else.\n\
         To call a tool: {{\"tool\": \"<tool name>\", \"arguments\": {{...}}}}\n\
         To give your final answer: {{\"final\": \"<answer>\"}}",
        serde_json::to_string_pretty(&tools).unwrap_or_default()
    );
    let system = match &req.system {
        Some(s) => format!("{s}\n\n{protocol}"),
        None => protocol,
    };
    let messages = req.messages.iter().map(flatten_message).collect();
    CompletionRequest {
        system: Some(system),
        messages,
        tools: Vec::new(),
        max_tokens: req.max_tokens,
        temperature: req.temperature,
    }
}

/// Earlier tool calls and results become plain text in JSON mode.
fn flatten_message(m: &Message) -> Message {
    let content = m
        .content
        .iter()
        .map(|p| match p {
            Part::Text { .. } => p.clone(),
            Part::ToolCall(c) => Part::Text {
                text: json!({"tool": c.name, "arguments": c.arguments}).to_string(),
            },
            Part::ToolResult {
                call_id,
                content,
                is_error,
            } => Part::Text {
                text: format!(
                    "Result of tool call {call_id}{}:\n{content}",
                    if *is_error { " (error)" } else { "" }
                ),
            },
        })
        .collect();
    Message {
        role: m.role,
        content,
    }
}

/// Interpret a JSON-mode reply as a normal response with text or a tool call.
pub fn parse_json_mode(
    resp: &CompletionResponse,
    tools: &[ToolSpec],
    call_index: usize,
) -> Result<CompletionResponse, String> {
    let raw = resp.text();
    let obj = extract_json_object(&raw).ok_or("reply did not contain a JSON object")?;
    let content;
    let stop_reason;
    if let Some(name) = obj.get("tool").and_then(Value::as_str) {
        let spec = tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| format!("unknown tool `{name}`"))?;
        let arguments = obj.get("arguments").cloned().unwrap_or_else(|| json!({}));
        check_required(spec, &arguments)?;
        content = vec![Part::ToolCall(ToolCall {
            id: format!("call_{call_index}"),
            name: name.to_owned(),
            arguments,
        })];
        stop_reason = StopReason::ToolUse;
    } else if let Some(answer) = obj.get("final") {
        let text = match answer {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        };
        content = vec![Part::Text { text }];
        stop_reason = StopReason::EndTurn;
    } else {
        return Err("JSON object has neither `tool` nor `final`".into());
    }
    Ok(CompletionResponse {
        content,
        usage: resp.usage,
        stop_reason,
    })
}

/// The follow-up message asking the model to correct an invalid reply.
pub fn correction(reply: &str, problem: &str) -> [Message; 2] {
    [
        Message {
            role: Role::Assistant,
            content: vec![Part::Text {
                text: reply.to_owned(),
            }],
        },
        Message::user(format!(
            "Your reply was not valid: {problem}. Reply again with exactly one JSON object and nothing else."
        )),
    ]
}

fn check_required(spec: &ToolSpec, args: &Value) -> Result<(), String> {
    let Some(obj) = args.as_object() else {
        return Err(format!(
            "arguments for `{}` must be a JSON object",
            spec.name
        ));
    };
    for key in spec
        .input_schema
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
    {
        if !obj.contains_key(key) {
            return Err(format!(
                "`{}` is missing required argument `{key}`",
                spec.name
            ));
        }
    }
    Ok(())
}

/// Find the first complete JSON object in model output. Handles code fences,
/// `<think>` blocks and text around the object.
pub fn extract_json_object(text: &str) -> Option<serde_json::Map<String, Value>> {
    let text = match (text.find("<think>"), text.find("</think>")) {
        (Some(_), Some(end)) => &text[end + "</think>".len()..],
        _ => text,
    };
    let bytes = text.as_bytes();
    let mut start = 0;
    while let Some(off) = text[start..].find('{') {
        let open = start + off;
        let (mut depth, mut in_str, mut escaped) = (0usize, false, false);
        for (i, &b) in bytes.iter().enumerate().skip(open) {
            if in_str {
                match b {
                    _ if escaped => escaped = false,
                    b'\\' => escaped = true,
                    b'"' => in_str = false,
                    _ => {}
                }
                continue;
            }
            match b {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        if let Ok(Value::Object(map)) = serde_json::from_str(&text[open..=i]) {
                            return Some(map);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        start = open + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Usage;

    fn tool() -> ToolSpec {
        ToolSpec {
            name: "report_finding".into(),
            description: "Report a bug".into(),
            input_schema: json!({"type": "object", "required": ["title", "line"],
                "properties": {"title": {"type": "string"}, "line": {"type": "integer"}}}),
        }
    }

    fn reply(text: &str) -> CompletionResponse {
        CompletionResponse {
            content: vec![Part::Text { text: text.into() }],
            usage: Usage::default(),
            stop_reason: StopReason::EndTurn,
        }
    }

    #[test]
    fn rewrites_tools_into_system_prompt() {
        let mut req = CompletionRequest::from_prompt("hi", 100);
        req.tools = vec![tool()];
        let out = to_json_mode(&req);
        assert!(out.tools.is_empty());
        assert!(out.system.unwrap().contains("report_finding"));
    }

    #[test]
    fn parses_tool_call_in_code_fence_with_thinking() {
        let r = reply(
            "<think>hmm {not json}</think>\n```json\n{\"tool\": \"report_finding\", \"arguments\": {\"title\": \"x}\", \"line\": 3}}\n```",
        );
        let out = parse_json_mode(&r, &[tool()], 0).unwrap();
        assert_eq!(out.stop_reason, StopReason::ToolUse);
        assert_eq!(out.tool_calls()[0].arguments["title"], "x}");
    }

    #[test]
    fn parses_final_answer() {
        let out = parse_json_mode(&reply(r#"Sure. {"final": "done"}"#), &[tool()], 0).unwrap();
        assert_eq!(out.text(), "done");
    }

    #[test]
    fn rejects_unknown_tool_and_missing_args() {
        assert!(
            parse_json_mode(
                &reply(r#"{"tool": "rm_rf", "arguments": {}}"#),
                &[tool()],
                0
            )
            .unwrap_err()
            .contains("unknown tool")
        );
        assert!(
            parse_json_mode(
                &reply(r#"{"tool": "report_finding", "arguments": {"title": "a"}}"#),
                &[tool()],
                0
            )
            .unwrap_err()
            .contains("line")
        );
        assert!(parse_json_mode(&reply("no json here"), &[tool()], 0).is_err());
    }
}
