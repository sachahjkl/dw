use thiserror::Error;

const FORBIDDEN_TOKENS: &[&str] = &[
    "insert", "update", "delete", "merge", "drop", "alter", "truncate", "exec", "execute",
    "create", "grant", "revoke",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqlGuardResult {
    pub is_allowed: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum SqlGuardError {
    #[error("Requete bloquee: {0}")]
    Blocked(String),
}

pub fn validate_read_only_sql(sql: &str) -> SqlGuardResult {
    if sql.trim().is_empty() {
        return SqlGuardResult {
            is_allowed: false,
            reason: Some("La requete SQL est vide.".into()),
        };
    }

    let cleaned = strip_comments(sql).trim().to_string();
    let starts_with_allowed = cleaned.starts_with("select")
        || cleaned.starts_with("SELECT")
        || cleaned.starts_with("with")
        || cleaned.starts_with("WITH")
        || cleaned.starts_with("sp_help")
        || cleaned.starts_with("SP_HELP");

    if !starts_with_allowed {
        return SqlGuardResult {
            is_allowed: false,
            reason: Some(
                "Seules les requetes SELECT/WITH et l'introspection read-only sont autorisees."
                    .into(),
            ),
        };
    }

    let lowered = cleaned.to_ascii_lowercase();
    for token in FORBIDDEN_TOKENS {
        let token_with_spaces = format!(" {} ", token);
        let token_with_newline = format!("\n{}\n", token);
        if lowered == *token
            || lowered.contains(&token_with_spaces)
            || lowered.contains(&token_with_newline)
            || lowered.starts_with(&format!("{} ", token))
            || lowered.ends_with(&format!(" {}", token))
        {
            return SqlGuardResult {
                is_allowed: false,
                reason: Some(format!(
                    "Mot-cle SQL interdit en mode read-only: {}.",
                    token.to_uppercase()
                )),
            };
        }
    }

    SqlGuardResult {
        is_allowed: true,
        reason: None,
    }
}

fn strip_comments(sql: &str) -> String {
    let mut without_line_comments = String::new();
    for line in sql.lines() {
        if let Some((before, _)) = line.split_once("--") {
            without_line_comments.push_str(before);
        } else {
            without_line_comments.push_str(line);
        }
        without_line_comments.push('\n');
    }

    let mut result = without_line_comments;
    while let Some(start) = result.find("/*") {
        if let Some(end_offset) = result[start + 2..].find("*/") {
            let end = start + 2 + end_offset + 2;
            result.replace_range(start..end, " ");
        } else {
            break;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_select() {
        assert!(validate_read_only_sql("select 1").is_allowed);
    }

    #[test]
    fn blocks_update() {
        let result = validate_read_only_sql("select 1; update t set x = 1");
        assert!(!result.is_allowed);
    }
}
