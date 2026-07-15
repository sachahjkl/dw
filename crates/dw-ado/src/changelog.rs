use crate::{
    AdoError, AdoToken, AzureDevOpsOptions, WorkItemSnapshot, get_related_work_item_ids,
    get_work_item_snapshot_authenticated, get_work_item_snapshots_authenticated,
    try_get_pull_request_work_item_ids,
};
use dw_core::{PullRequestId, WorkItemId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const RELATION_HIERARCHY_REVERSE: &str = "System.LinkTypes.Hierarchy-Reverse";
pub const RELATION_HIERARCHY_FORWARD: &str = "System.LinkTypes.Hierarchy-Forward";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkItemGroup {
    pub parent: WorkItemSnapshot,
    pub items: Vec<WorkItemSnapshot>,
}

pub fn extract_work_item_ids_from_commit_messages(commit_log: &str) -> Vec<WorkItemId> {
    let mut ids = Vec::new();
    for (index, _) in commit_log.match_indices('#') {
        let id = commit_log[index + 1..]
            .chars()
            .take_while(|ch| ch.is_ascii_digit())
            .collect::<String>();
        let id = WorkItemId::from(id);
        if !id.as_str().is_empty() && !ids.iter().any(|existing| existing == &id) {
            ids.push(id);
        }
    }
    ids
}

pub fn group_work_items_by_parent(
    options: &AzureDevOpsOptions,
    items: &[WorkItemSnapshot],
    token: &AdoToken,
) -> Result<Vec<WorkItemGroup>, AdoError> {
    let mut groups: BTreeMap<WorkItemId, Vec<WorkItemSnapshot>> = BTreeMap::new();
    let mut parents: BTreeMap<WorkItemId, WorkItemSnapshot> = BTreeMap::new();

    for item in items {
        let parent_id = get_related_work_item_ids(
            options,
            item.id.as_str(),
            RELATION_HIERARCHY_REVERSE,
            token,
        )?
        .into_iter()
        .next()
        .unwrap_or_else(|| item.id.clone());
        if parent_id == item.id {
            parents.insert(parent_id.clone(), item.clone());
        } else if !parents.contains_key(&parent_id) {
            parents.insert(
                parent_id.clone(),
                get_work_item_snapshot_authenticated(options, parent_id.as_str(), token)?,
            );
        }

        let children = groups.entry(parent_id.clone()).or_default();
        if parent_id != item.id {
            children.push(item.clone());
        }
    }

    Ok(groups
        .into_iter()
        .filter_map(|(parent_id, mut items)| {
            items.sort_by(|left, right| left.id.cmp(&right.id));
            Some(WorkItemGroup {
                parent: parents.remove(&parent_id)?,
                items,
            })
        })
        .collect())
}

pub fn get_work_item_ids_from_pull_requests(
    options: &AzureDevOpsOptions,
    repositories: &[String],
    pull_request_ids: &[PullRequestId],
    token: &AdoToken,
) -> Result<Vec<WorkItemId>, AdoError> {
    if repositories.is_empty() {
        return Err(AdoError::InvalidInput(
            "PR mode requires an explicit repository, or a project with configured AzureDevOpsRepository entries.".into(),
        ));
    }

    let mut ids = Vec::new();
    for pull_request_id in pull_request_ids {
        let numeric_pull_request_id = pull_request_id.as_str().parse::<i64>().map_err(|_| {
            AdoError::InvalidInput(format!("Invalid pull request ID: {pull_request_id}"))
        })?;
        let mut matches = Vec::new();
        for repository in repositories {
            if let Some(work_item_ids) = try_get_pull_request_work_item_ids(
                options,
                repository,
                numeric_pull_request_id,
                token,
            )? {
                matches.push((repository.clone(), work_item_ids));
            }
        }

        match matches.len() {
            0 => {
                return Err(AdoError::Request(format!(
                    "Pull request #{pull_request_id} was not found in tested Azure DevOps repos: {}",
                    repositories.join(", ")
                )));
            }
            1 => ids.extend(matches.remove(0).1),
            _ => {
                return Err(AdoError::InvalidInput(format!(
                    "Pull request #{pull_request_id} was found in multiple repos ({}). Specify the repository.",
                    matches
                        .into_iter()
                        .map(|item| item.0)
                        .collect::<Vec<_>>()
                        .join(", ")
                )));
            }
        }
    }

    let mut seen = BTreeSet::new();
    Ok(ids
        .into_iter()
        .filter(|id| seen.insert(id.clone()))
        .collect())
}

pub fn load_changelog_items(
    options: &AzureDevOpsOptions,
    work_item_ids: &[WorkItemId],
    token: &AdoToken,
) -> Result<Vec<WorkItemSnapshot>, AdoError> {
    get_work_item_snapshots_authenticated(options, work_item_ids, token)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_work_item_ids_from_commit_messages_reads_ids_in_order_and_dedupes() {
        let commit_log = "fix(#104 #105): corriger le calcul\u{1e}refactor(#104 #107): simplifier";

        let ids = extract_work_item_ids_from_commit_messages(commit_log);

        assert_eq!(
            ids,
            vec![
                WorkItemId::from("104"),
                WorkItemId::from("105"),
                WorkItemId::from("107")
            ]
        );
        assert_eq!(
            ids.iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(" "),
            "104 105 107"
        );
    }
}
