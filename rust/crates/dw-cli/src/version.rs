pub(crate) fn informational_version() -> String {
    let version = option_env!("DW_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    match option_env!("DW_COMMIT").filter(|value| !value.trim().is_empty()) {
        Some(commit) => format!("{version}+{}", short_commit(commit)),
        None => version.into(),
    }
}

fn short_commit(commit: &str) -> &str {
    commit.get(..7).unwrap_or(commit)
}

#[cfg(test)]
mod tests {
    use super::short_commit;

    #[test]
    fn short_commit_keeps_short_values() {
        assert_eq!(short_commit("abc"), "abc");
    }

    #[test]
    fn short_commit_truncates_long_values() {
        assert_eq!(short_commit("abcdef123456"), "abcdef1");
    }
}
