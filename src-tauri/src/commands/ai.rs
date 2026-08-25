// Shared OpenRouter client + helpers for the AI features (Ask, Insights).
// Mirrors the Family Finance integration: same provider and model so the user's
// existing OpenRouter key and expectations carry over.

/// Default model — matches the Family Finance app so an existing key/expectations
/// carry over. Overridable per-install via the model picker in Settings.
pub const MODEL: &str = "deepseek/deepseek-v4-flash";

/// How much of `max_tokens` the model may spend on its private reasoning trace.
///
/// OpenRouter draws reasoning tokens from the same `max_tokens` allowance as the
/// visible reply, so an unbounded trace can swallow the entire budget and come
/// back as `content: null` with `finish_reason: "length"` — the model thought
/// until it ran out and never wrote an answer. Reserving half the budget (and no
/// more than 1500 tokens) guarantees the reply has room. Models that do not
/// reason ignore the field.
fn reasoning_budget(max_tokens: u32) -> u32 {
    (max_tokens / 2).clamp(128, 1500)
}

/// POST a single-prompt chat completion to OpenRouter and return the message text.
/// Uses the model chosen in Settings, falling back to [`MODEL`].
pub async fn call_openrouter(
    api_key: &str,
    prompt: &str,
    temperature: f64,
    max_tokens: u32,
) -> Result<String, String> {
    let model = crate::commands::settings::model();
    let client = reqwest::Client::new();
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
        "max_tokens": max_tokens,
        "reasoning": {"max_tokens": reasoning_budget(max_tokens)},
    });

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("API request failed: {}", e))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {}", e))?;

    if !status.is_success() {
        return Err(format!("OpenRouter API error ({}): {}", status, text));
    }

    interpret_response(&text)
}

#[derive(serde::Deserialize)]
struct Resp {
    choices: Vec<Choice>,
}

#[derive(serde::Deserialize)]
struct Choice {
    message: Msg,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(serde::Deserialize)]
struct Msg {
    // Null whenever the model emitted no visible output. That is a normal API
    // outcome, not malformed JSON, so it must not fail deserialization — the
    // reason lives in the sibling `finish_reason`.
    #[serde(default)]
    content: Option<String>,
}

/// Turn a successful HTTP body into the reply text, or into an error a user can
/// act on. Split out from the request so it can be tested without a network call.
fn interpret_response(text: &str) -> Result<String, String> {
    let parsed: Resp = serde_json::from_str(text)
        .map_err(|e| format!("Failed to parse API response: {} - {}", e, truncate(text)))?;

    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| "The API returned no choices.".to_string())?;

    match choice.message.content.as_deref().map(str::trim) {
        Some(content) if !content.is_empty() => Ok(content.to_string()),
        _ => Err(no_content_message(choice.finish_reason.as_deref())),
    }
}

/// Explain an empty reply in terms the Ask screen can show the user directly.
fn no_content_message(finish_reason: Option<&str>) -> String {
    match finish_reason {
        Some("length") => {
            "The model used its whole token budget before writing an answer.              Try a simpler or more specific question."
                .to_string()
        }
        Some("content_filter") => "The provider blocked this request.".to_string(),
        Some(other) => format!("The model returned no answer (finish reason: {}).", other),
        None => "The model returned no answer.".to_string(),
    }
}

/// Keep a failed-response excerpt short enough to show in the UI. A reasoning
/// model's raw body runs to several KB, which is unreadable in a toast.
fn truncate(s: &str) -> String {
    const MAX: usize = 300;
    if s.chars().count() <= MAX {
        return s.to_string();
    }
    let head: String = s.chars().take(MAX).collect();
    format!("{}… ({} bytes total)", head, s.len())
}

/// Strip a leading ```json / ``` fence and trailing ``` the model may add.
pub fn strip_code_fences(content: &str) -> &str {
    content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shape that broke the Ask screen: a reasoning model that spent its whole
    /// allowance thinking, so `content` is null and `finish_reason` is "length".
    const NO_CONTENT: &str = r#"{
      "id": "gen-1", "model": "deepseek/deepseek-v4-flash",
      "choices": [{
        "index": 0, "finish_reason": "length", "native_finish_reason": "length",
        "message": {"role": "assistant", "content": null, "refusal": null,
                    "reasoning": "thinking, and thinking, and thinking"}
      }],
      "usage": {"completion_tokens": 1024, "completion_tokens_details": {"reasoning_tokens": 1024}}
    }"#;

    #[test]
    fn null_content_reports_the_token_budget_not_a_parse_error() {
        let err = interpret_response(NO_CONTENT).unwrap_err();
        assert!(err.contains("token budget"), "unhelpful message: {err}");
        assert!(!err.contains("invalid type"), "leaked a serde error: {err}");
        assert!(!err.contains("reasoning"), "leaked the raw body: {err}");
    }

    #[test]
    fn normal_reply_is_returned_trimmed() {
        let body = r#"{"choices":[{"finish_reason":"stop",
            "message":{"role":"assistant","content":"  SELECT 1  "}}]}"#;
        assert_eq!(interpret_response(body).unwrap(), "SELECT 1");
    }

    #[test]
    fn whitespace_only_content_is_treated_as_empty() {
        let body = r#"{"choices":[{"finish_reason":"stop",
            "message":{"role":"assistant","content":"   "}}]}"#;
        assert!(interpret_response(body).is_err());
    }

    #[test]
    fn missing_finish_reason_still_gives_a_readable_error() {
        let body = r#"{"choices":[{"message":{"role":"assistant","content":null}}]}"#;
        let err = interpret_response(body).unwrap_err();
        assert_eq!(err, "The model returned no answer.");
    }

    #[test]
    fn malformed_body_excerpt_is_short() {
        let err = interpret_response(&"x".repeat(9000)).unwrap_err();
        assert!(err.len() < 500, "error was {} chars", err.len());
        assert!(err.contains("9000 bytes total"));
    }

    #[test]
    fn reasoning_never_claims_the_whole_budget() {
        for max in [1024u32, 2048, 3072, 4096, 8192] {
            let r = reasoning_budget(max);
            assert!(r <= max / 2, "{r} of {max} leaves too little for the reply");
            assert!(max - r >= 512, "only {} tokens left for the reply", max - r);
        }
        // Tiny budgets still reserve something rather than underflowing to zero.
        assert_eq!(reasoning_budget(100), 128);
    }
}
