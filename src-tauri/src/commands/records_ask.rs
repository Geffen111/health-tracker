// "Ask your records" — free-text Q&A over the Health Records vault prose.
//
// Unlike the structured Ask (text-to-SQL over the log DB), this answers
// qualitative questions ("summarise my neurology history") by retrieving the
// most relevant notes with simple keyword scoring and sending their text to the
// model to answer with citations. No embeddings/index — keyword retrieval keeps
// it dependency-free and fast for a vault of this size.
//
// NOTE: this sends raw note content to OpenRouter (user-chosen; see labs.rs).

use crate::commands::ai::call_openrouter;
use crate::commands::{settings, vault};
use serde::Serialize;

const TOP_K: usize = 6;
const PER_NOTE_CHARS: usize = 1800;

#[derive(Serialize)]
pub struct SourceRef {
    pub title: String,
    pub rel_path: String,
}

#[derive(Serialize)]
pub struct RecordsAnswer {
    pub answer: String,
    pub sources: Vec<SourceRef>,
}

const STOPWORDS: &[&str] = &[
    "the", "and", "for", "are", "was", "were", "what", "when", "which", "with", "have", "has",
    "had", "did", "does", "do", "my", "me", "i", "is", "of", "to", "in", "on", "a", "an", "any",
    "all", "how", "show", "tell", "about", "from", "that", "this", "give", "list",
];

#[tauri::command]
pub async fn ask_records(question: String) -> Result<RecordsAnswer, String> {
    let api_key = settings::get_api_key()
        .await?
        .ok_or_else(|| "OpenRouter API key not configured. Add your key in Settings.".to_string())?;

    let q = question.trim();
    if q.is_empty() {
        return Err("Please enter a question.".to_string());
    }

    let terms = query_terms(q);
    let notes = vault::walk_notes();

    // Score and rank notes by keyword overlap.
    let mut scored: Vec<(i64, &vault::RawNote)> = notes
        .iter()
        .map(|n| (score_note(n, &terms), n))
        .filter(|(s, _)| *s > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored.truncate(TOP_K);

    if scored.is_empty() {
        return Ok(RecordsAnswer {
            answer: "I couldn't find any records matching that question. Try different wording, or browse the notes directly.".to_string(),
            sources: vec![],
        });
    }

    let sources: Vec<SourceRef> = scored
        .iter()
        .map(|(_, n)| SourceRef { title: n.title.clone(), rel_path: n.rel_path.clone() })
        .collect();

    // Build the context from the selected notes.
    let mut context = String::new();
    for (_, n) in &scored {
        context.push_str("### ");
        context.push_str(&n.title);
        context.push_str(&format!(" ({})\n", n.rel_path));
        context.push_str(truncate_chars(&n.body, PER_NOTE_CHARS));
        context.push_str("\n\n");
    }

    let prompt = format!(
        r#"You are a careful assistant answering questions about one person's personal health records.
Use ONLY the notes provided below — do not invent facts. If the notes don't contain the answer, say so plainly.
Cite the note titles you drew from. Do not give medical advice or diagnoses; describe what the records say.

Question: "{question}"

Notes:
{context}

Answer concisely (a few sentences, or a short list if appropriate)."#,
        question = q,
        context = context,
    );

    let answer = call_openrouter(&api_key, &prompt, 0.2, 1024).await?;
    Ok(RecordsAnswer { answer, sources })
}

fn query_terms(q: &str) -> Vec<String> {
    q.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() >= 3 && !STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

fn score_note(n: &vault::RawNote, terms: &[String]) -> i64 {
    if terms.is_empty() {
        return 0;
    }
    let title = n.title.to_lowercase();
    let folder = n.folder.to_lowercase();
    let tags = n.tags.join(" ").to_lowercase();
    let body = n.body.to_lowercase();
    let mut score = 0i64;
    for t in terms {
        score += 3 * title.matches(t.as_str()).count() as i64;
        score += 3 * tags.matches(t.as_str()).count() as i64;
        score += 2 * folder.matches(t.as_str()).count() as i64;
        score += body.matches(t.as_str()).count() as i64;
    }
    score
}

fn truncate_chars(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drops_stopwords_and_short() {
        let t = query_terms("What is my neurology history?");
        assert!(t.contains(&"neurology".to_string()));
        assert!(t.contains(&"history".to_string()));
        assert!(!t.contains(&"is".to_string()));
        assert!(!t.contains(&"my".to_string()));
    }
}
