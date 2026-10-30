//! A ranked search over the documentation, for finding what a change is
//! about before knowing which links to follow.

use crate::location::Span;
use crate::workspace::Workspace;

/// A match in a unit's name (its path and title) outweighs this many
/// matches in its text.
const NAME_WEIGHT: f64 = 3.0;

pub struct Found {
    pub score: f64,
    pub span: Span,
    pub title: String,
    pub text: String,
    /// The probability that the unit is relevant, once re-ranked.
    pub relevance: Option<f64>,
}

/// The parts of the documentation matching [`query`](lynk:rust.argument), best
/// first, searching
/// every document's prose unit by unit, so a match in a long file points
/// at the paragraph it is in.
pub fn find(workspace: &Workspace, query: &str) -> Vec<Found> {
    let terms = stems(query);
    if terms.is_empty() {
        return Vec::new();
    }
    let mut found = Vec::new();
    for linked in &workspace.documents {
        let document = &linked.document;
        let path = document.path.to_string_lossy();
        for unit in &document.units {
            let name = format!("{path} {}", unit.title);
            let text = &document.prose[unit.range.clone()];
            let score = score(&terms, &stems(&name), &stems(text));
            if score > 0.0 {
                found.push(Found {
                    score,
                    text: text.to_owned(),
                    span: document.span(unit.range.clone()),
                    title: if unit.title.is_empty() {
                        path.to_string()
                    } else {
                        unit.title.clone()
                    },
                    relevance: None,
                });
            }
        }
    }
    found.sort_by(|first, second| second.score.total_cmp(&first.score));
    found
}

/// More matches score higher, with diminishing returns; units matching only
/// some of the terms are scaled down by the share they match.
fn score(terms: &[String], name: &[String], text: &[String]) -> f64 {
    let mut total = 0.0;
    let mut matched_terms = 0;
    for term in terms {
        let name_matches = name.iter().filter(|word| *word == term).count() as f64;
        let text_matches = text.iter().filter(|word| *word == term).count() as f64;
        if name_matches + text_matches == 0.0 {
            continue;
        }
        matched_terms += 1;
        total += NAME_WEIGHT * name_matches + text_matches.sqrt();
    }
    total * matched_terms as f64 / terms.len() as f64
}

/// Lowercase words with common English suffixes removed, so `parsing`,
/// `parses` and `parse` compare equal.
fn stems(text: &str) -> Vec<String> {
    text.split(|character: char| !character.is_alphanumeric())
        .filter(|word| word.len() > 1)
        .map(|word| stem(&word.to_lowercase()))
        .collect()
}

fn stem(word: &str) -> String {
    let mut stem = word;
    for suffix in ["ing", "ed", "es", "s"] {
        if let Some(stripped) = stem.strip_suffix(suffix)
            && stripped.chars().count() >= 3
        {
            stem = stripped;
            break;
        }
    }
    if let Some(stripped) = stem.strip_suffix('e')
        && stripped.chars().count() >= 3
    {
        stem = stripped;
    }
    stem.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inflections_share_a_stem() {
        assert_eq!(stems("parsing parses parse"), vec!["pars", "pars", "pars"]);
        assert_eq!(stems("Comments comment"), vec!["comment", "comment"]);
    }

    #[test]
    fn a_name_match_outweighs_a_text_match() {
        let terms = stems("rust");
        assert!(score(&terms, &stems("rust"), &[]) > score(&terms, &[], &stems("rust")));
    }

    #[test]
    fn more_matches_score_higher() {
        let terms = stems("rust");
        assert!(score(&terms, &[], &stems("rust rust")) > score(&terms, &[], &stems("rust")));
    }

    #[test]
    fn matching_every_term_beats_matching_one_term_often() {
        let terms = stems("rust comment");
        assert!(score(&terms, &[], &stems("rust comment")) > score(&terms, &[], &stems("rust rust rust")));
    }
}
