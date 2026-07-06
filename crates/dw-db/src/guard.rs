use serde::Serialize;

const FORBIDDEN_TOKENS: &[&str] = &[
    "insert", "update", "delete", "merge", "drop", "alter", "truncate", "exec", "execute",
    "create", "grant", "revoke",
];

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SqlGuardResult {
    pub is_allowed: bool,
    pub reason: Option<String>,
}

impl SqlGuardResult {
    fn allowed() -> Self {
        Self {
            is_allowed: true,
            reason: None,
        }
    }

    fn blocked(reason: impl Into<String>) -> Self {
        Self {
            is_allowed: false,
            reason: Some(reason.into()),
        }
    }
}

pub fn validate_read_only_sql(sql: &str) -> SqlGuardResult {
    if sql.trim().is_empty() {
        return SqlGuardResult::blocked("La requête SQL est vide.");
    }

    let cleaned = strip_comments(sql).trim().to_string();
    if !starts_with_readonly_verb(&cleaned) {
        return SqlGuardResult::blocked(
            "Seules les requêtes SELECT/WITH et l'introspection read-only sont autorisées.",
        );
    }

    let lowered = cleaned.to_ascii_lowercase();
    for token in FORBIDDEN_TOKENS {
        if contains_word(&lowered, token) {
            return SqlGuardResult::blocked(format!(
                "Mot-cle SQL interdit en mode read-only: {}.",
                token.to_uppercase()
            ));
        }
    }

    SqlGuardResult::allowed()
}

fn starts_with_readonly_verb(sql: &str) -> bool {
    let lowered = sql.to_ascii_lowercase();
    lowered.starts_with("select") || lowered.starts_with("with") || lowered.starts_with("sp_help")
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    haystack.match_indices(needle).any(|(index, _)| {
        is_boundary(haystack, index) && is_boundary(haystack, index + needle.len())
    })
}

fn is_boundary(value: &str, index: usize) -> bool {
    if index == 0 || index >= value.len() {
        return true;
    }
    let byte = value.as_bytes()[index];
    let previous = value.as_bytes()[index - 1];
    (!byte.is_ascii_alphanumeric() && byte != b'_')
        || (!previous.is_ascii_alphanumeric() && previous != b'_')
}

fn strip_comments(sql: &str) -> String {
    let mut without_block_comments = sql.to_string();
    while let Some(start) = without_block_comments.find("/*") {
        let Some(end_offset) = without_block_comments[start + 2..].find("*/") else {
            break;
        };
        let end = start + 2 + end_offset + 2;
        without_block_comments.replace_range(start..end, " ");
    }

    without_block_comments
        .lines()
        .map(|line| {
            line.split_once("--")
                .map(|(before, _)| before)
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_readonly_queries() {
        for sql in [
            "select top 10 * from dbo.Users",
            "-- comment\r\nselect 1",
            "with cte as (select 1 as Id) select * from cte",
        ] {
            assert!(validate_read_only_sql(sql).is_allowed, "{sql}");
        }
    }

    #[test]
    fn blocks_dangerous_queries() {
        for sql in [
            "delete from dbo.Users",
            "select * from dbo.Users; drop table dbo.Users",
            "exec dbo.DoSomething",
            "update dbo.Users set Name = 'x'",
        ] {
            let result = validate_read_only_sql(sql);
            assert!(!result.is_allowed, "{sql}");
            assert!(result.reason.is_some());
        }
    }
}
