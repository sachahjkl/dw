pub fn is_final_state(work_item_type: Option<&str>, state: Option<&str>) -> bool {
    let normalized_state = normalize_state_or_type(state);
    if normalized_state.is_empty() {
        return false;
    }
    let normalized_type = normalize_state_or_type(work_item_type);
    let final_states = [
        "valide",
        "validé",
        "cloture",
        "clôturé",
        "abandonne",
        "abandonné",
    ];
    let final_states_without_validated = ["cloture", "clôturé", "abandonne", "abandonné"];
    match normalized_type.as_str() {
        "user story" | "anomalie" => final_states.contains(&normalized_state.as_str()),
        "bug" | "activite" | "activité" => {
            final_states_without_validated.contains(&normalized_state.as_str())
        }
        _ => final_states.contains(&normalized_state.as_str()),
    }
}

fn normalize_state_or_type(value: Option<&str>) -> String {
    value
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .replace(['é', 'è', 'ê'], "e")
        .replace(['à', 'â'], "a")
        .replace('ô', "o")
        .replace(['û', 'ù'], "u")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn final_state_matches_workspace_rules() {
        assert!(is_final_state(Some("User Story"), Some("Validé")));
        assert!(!is_final_state(Some("Bug"), Some("Validé")));
        assert!(is_final_state(Some("Bug"), Some("Clôturé")));
    }
}
