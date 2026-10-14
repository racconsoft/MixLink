//! Tolerant name resolution: exact → prefix → substring → fuzzy (Jaro-Winkler).
use thiserror::Error;

#[derive(Debug, Clone, Copy)]
pub struct Candidate<'a> {
    pub id: &'a str,
    pub name: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResolveError {
    #[error("nothing matches '{query}'")]
    NotFound { query: String, options: Vec<String> },
    #[error("'{query}' is ambiguous")]
    Ambiguous { query: String, options: Vec<String> },
}

impl ResolveError {
    pub fn options(&self) -> &[String] {
        match self {
            Self::NotFound { options, .. } | Self::Ambiguous { options, .. } => options,
        }
    }
}

const FUZZY_THRESHOLD: f64 = 0.85;
const FUZZY_TIE: f64 = 0.02;

pub fn resolve<'a>(query: &str, candidates: &[Candidate<'a>]) -> Result<&'a str, ResolveError> {
    let q = query.trim().to_lowercase();
    let names = |cs: &[&Candidate<'a>]| cs.iter().map(|c| c.name.to_string()).collect::<Vec<_>>();
    let all: Vec<&Candidate<'a>> = candidates.iter().collect();
    if q.is_empty() {
        return Err(ResolveError::NotFound {
            query: query.into(),
            options: names(&all),
        });
    }
    let lower = |s: &str| s.to_lowercase();

    if let Some(c) = candidates
        .iter()
        .find(|c| lower(c.id) == q || lower(c.name) == q)
    {
        return Ok(c.id);
    }

    let prefix: Vec<_> = candidates
        .iter()
        .filter(|c| lower(c.name).starts_with(&q) || lower(c.id).starts_with(&q))
        .collect();
    if let Some(result) = unique(query, &prefix) {
        return result;
    }

    let substring: Vec<_> = candidates
        .iter()
        .filter(|c| lower(c.name).contains(&q) || lower(c.id).contains(&q))
        .collect();
    if let Some(result) = unique(query, &substring) {
        return result;
    }

    let mut scored: Vec<(f64, &Candidate<'a>)> = candidates
        .iter()
        .map(|c| {
            let score = strsim::jaro_winkler(&q, &lower(c.name))
                .max(strsim::jaro_winkler(&q, &lower(c.id)));
            (score, c)
        })
        .filter(|(score, _)| *score >= FUZZY_THRESHOLD)
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    match scored.as_slice() {
        [] => Err(ResolveError::NotFound {
            query: query.into(),
            options: names(&all),
        }),
        [(best, c), rest @ ..] => {
            let ties: Vec<&Candidate<'a>> = rest
                .iter()
                .filter(|(s, _)| best - s < FUZZY_TIE)
                .map(|(_, c)| *c)
                .collect();
            if ties.is_empty() {
                Ok(c.id)
            } else {
                let mut matches = vec![*c];
                matches.extend(ties);
                Err(ResolveError::Ambiguous {
                    query: query.into(),
                    options: names(&matches),
                })
            }
        }
    }
}

fn unique<'a>(query: &str, matches: &[&Candidate<'a>]) -> Option<Result<&'a str, ResolveError>> {
    match matches {
        [] => None,
        [only] => Some(Ok(only.id)),
        many => Some(Err(ResolveError::Ambiguous {
            query: query.into(),
            options: many.iter().map(|c| c.name.to_string()).collect(),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channels() -> Vec<Candidate<'static>> {
        vec![
            Candidate {
                id: "system",
                name: "System",
            },
            Candidate {
                id: "game",
                name: "Game",
            },
            Candidate {
                id: "browser",
                name: "Browser",
            },
            Candidate {
                id: "voice-chat",
                name: "Voice Chat",
            },
            Candidate {
                id: "music",
                name: "Music",
            },
        ]
    }

    #[test]
    fn exact_match_ignores_case_on_id_and_name() {
        assert_eq!(resolve("music", &channels()), Ok("music"));
        assert_eq!(resolve("VOICE CHAT", &channels()), Ok("voice-chat"));
    }

    #[test]
    fn unique_prefix_matches() {
        assert_eq!(resolve("voice", &channels()), Ok("voice-chat"));
    }

    #[test]
    fn unique_substring_matches() {
        assert_eq!(resolve("chat", &channels()), Ok("voice-chat"));
    }

    #[test]
    fn fuzzy_match_tolerates_typos() {
        assert_eq!(resolve("musik", &channels()), Ok("music"));
    }

    #[test]
    fn ambiguous_prefix_lists_matches() {
        let cands = [
            Candidate {
                id: "game",
                name: "Game",
            },
            Candidate {
                id: "game-capture",
                name: "Game Capture",
            },
        ];
        let err = resolve("gam", &cands).unwrap_err();
        assert!(matches!(err, ResolveError::Ambiguous { .. }));
        assert_eq!(
            err.options(),
            &["Game".to_string(), "Game Capture".to_string()]
        );
    }

    #[test]
    fn unknown_lists_all_options() {
        let err = resolve("xyz", &channels()).unwrap_err();
        assert!(matches!(err, ResolveError::NotFound { .. }));
        assert_eq!(err.options().len(), 5);
    }

    #[test]
    fn empty_query_is_not_found() {
        assert!(matches!(
            resolve("  ", &channels()),
            Err(ResolveError::NotFound { .. })
        ));
    }
}
