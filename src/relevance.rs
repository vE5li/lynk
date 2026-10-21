//! Re-ranks `find`'s best word matches by asking TypeSafe whether each one
//! is relevant to the task; <[markdown.section
//! adrs/find-re-ranks-with-typesafe.md "Decision"]> says why.

use std::time::Duration;

use serde_json::{Value, json};

use crate::find::Found;

/// The environment variable holding the key; without it, results keep
/// their word scores.
pub const API_KEY_VARIABLE: &str = "TYPESAFE_API_KEY";
/// How many of the best word matches are re-ranked.
pub const SHORTLIST_LENGTH: usize = 30;

const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
const MODEL: &str = "jev-latest";
/// Longer passages are cut; their beginning carries the heading and gist.
const PASSAGE_CHARACTERS: usize = 3000;
const TIMEOUT: Duration = Duration::from_secs(10);
const QUESTION: &str = "Is `passage` relevant to someone about to work on the task described in the state?";

/// The probability that each passage is relevant to
/// <[rust.argument task]>, in order.
pub fn relevance(api_key: &str, task: &str, passages: &[&str]) -> Result<Vec<f64>, String> {
    let questions: serde_json::Map<String, Value> = passages
        .iter()
        .enumerate()
        .map(|(index, passage)| {
            let passage: String = passage.chars().take(PASSAGE_CHARACTERS).collect();
            let question = json!({
                "type": "noul",
                "instructions": { "passage": passage, "question": QUESTION },
            });
            (question_id(index), question)
        })
        .collect();
    let request = json!({ "model": MODEL, "state": { "task": task }, "questions": questions });
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(TIMEOUT)).build().into();
    let mut response = agent
        .post(ENDPOINT)
        .header("Authorization", &format!("Bearer {api_key}"))
        .send_json(&request)
        .map_err(|error| error.to_string())?;
    let body: Value = response.body_mut().read_json().map_err(|error| error.to_string())?;
    (0..passages.len())
        .map(|index| {
            body["answers"][question_id(index)]["noul"]
                .as_f64()
                .ok_or_else(|| format!("no answer for passage {index}: {body}"))
        })
        .collect()
}

fn question_id(index: usize) -> String {
    format!("passage{index}")
}

/// Orders the shortlist by relevance; results past it are dropped, since
/// they were not judged.
pub fn re_rank(found: &mut Vec<Found>, relevances: &[f64]) {
    found.truncate(relevances.len());
    for (result, relevance) in found.iter_mut().zip(relevances) {
        result.relevance = Some(*relevance);
    }
    found.sort_by(|first, second| second.relevance.unwrap_or(0.0).total_cmp(&first.relevance.unwrap_or(0.0)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::location::Span;

    fn found(title: &str) -> Found {
        Found {
            score: 1.0,
            span: Span::whole_file(std::path::Path::new(title)),
            title: title.to_owned(),
            text: String::new(),
            relevance: None,
        }
    }

    #[test]
    fn orders_by_relevance_and_drops_what_was_not_judged() {
        let mut results = vec![found("first"), found("second"), found("unjudged")];
        re_rank(&mut results, &[0.2, 0.9]);
        let titles: Vec<&str> = results.iter().map(|result| result.title.as_str()).collect();
        assert_eq!(titles, vec!["second", "first"]);
    }
}
