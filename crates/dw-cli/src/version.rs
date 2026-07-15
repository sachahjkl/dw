pub(crate) fn informational_version() -> String {
    let version = env!("DW_VERSION");
    match option_env!("DW_COMMIT").filter(|value| !value.trim().is_empty()) {
        Some(commit) => format!("{version}+{}", short_commit(commit)),
        None => version.into(),
    }
}

pub(crate) const PACKAGE_VERSION: &str = env!("DW_VERSION");

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
        assert_eq!(short_commit("abcdef123"), "abcdef1");
    }
}
