use azure_devops_rust_api::wit::models::Wiql;

pub const FIELD_ID: &str = "System.Id";
pub const FIELD_TEAM_PROJECT: &str = "System.TeamProject";
pub const FIELD_ASSIGNED_TO: &str = "System.AssignedTo";
pub const FIELD_CHANGED_DATE: &str = "System.ChangedDate";

pub fn assigned_work_items_query() -> String {
    format!(
        "select [{FIELD_ID}]
from WorkItems
where [{FIELD_TEAM_PROJECT}] = @project
  and [{FIELD_ASSIGNED_TO}] = @Me
order by [{FIELD_CHANGED_DATE}] desc"
    )
}

pub fn assigned_work_items() -> Wiql {
    Wiql {
        query: Some(assigned_work_items_query()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assigned_work_items_query_matches_azure_wiql_shape() {
        let query = assigned_work_items_query();

        assert!(query.contains(&format!("select [{FIELD_ID}]\nfrom WorkItems")));
        assert!(query.contains(&format!("WorkItems\nwhere [{FIELD_TEAM_PROJECT}]")));
        assert!(query.contains(&format!("@project\n  and [{FIELD_ASSIGNED_TO}]")));
        assert!(query.contains(&format!("order by [{FIELD_CHANGED_DATE}] desc")));
        assert!(!query.contains("[System.Id]from"));
        assert!(!query.contains("WorkItemswhere"));
    }

    #[test]
    fn assigned_work_items_returns_sdk_model() {
        assert_eq!(
            assigned_work_items().query.as_deref(),
            Some(assigned_work_items_query().as_str())
        );
    }
}
