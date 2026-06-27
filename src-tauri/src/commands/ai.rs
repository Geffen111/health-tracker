// Shared OpenRouter client + helpers for the AI features (Ask, Insights).
// Mirrors the Family Finance integration: same provider and model so the user's
// existing OpenRouter key and expectations carry over.

/// Default model — matches the Family Finance app so an existing key/expectations
/// carry over. Overridable per-install via the model picker in Settings.
pub const MODEL: &str = "deepseek/deepseek-v4-flash";

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

    #[derive(serde::Deserialize)]
    struct Resp {
        choices: Vec<Choice>,
    }
    #[derive(serde::Deserialize)]
    struct Choice {
        message: Msg,
    }
    #[derive(serde::Deserialize)]
    struct Msg {
        content: String,
    }

    let parsed: Resp = serde_json::from_str(&text)
        .map_err(|e| format!("Failed to parse API response: {} - {}", e, text))?;
    parsed
        .choices
        .into_iter()
        .next()
        .map(|c| c.message.content)
        .ok_or_else(|| "No choices in API response".to_string())
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
