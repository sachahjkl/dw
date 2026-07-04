use crate::{AzureDevOpsOptions, DEFAULT_API_VERSION};

pub fn expanded_work_item_url(options: &AzureDevOpsOptions, work_item_id: &str) -> String {
    format!(
        "{}/{}/_apis/wit/workitems/{}?$expand=all&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id,
        api_version(options)
    )
}

pub fn work_item_comments_url(
    options: &AzureDevOpsOptions,
    work_item_id: &str,
    top: u32,
) -> String {
    format!(
        "{}/{}/_apis/wit/workItems/{}/comments?$top={}&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id,
        top,
        api_version(options)
    )
}

pub fn work_item_url(options: &AzureDevOpsOptions, work_item_id: &str) -> String {
    format!(
        "{}/{}/_apis/wit/workitems/{}?api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id,
        api_version(options)
    )
}

pub fn work_items_batch_url(options: &AzureDevOpsOptions) -> String {
    format!(
        "{}/{}/_apis/wit/workitemsbatch?api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        api_version(options)
    )
}

pub fn work_item_api_url(options: &AzureDevOpsOptions, work_item_id: &str) -> String {
    format!(
        "{}/{}/_apis/wit/workItems/{}",
        options.organization.trim_end_matches('/'),
        options.project,
        work_item_id
    )
}

pub fn work_item_web_url(options: &AzureDevOpsOptions, work_item_id: &str) -> String {
    format!(
        "{}/{}/_workitems/edit/{}",
        options.organization.trim_end_matches('/'),
        encode_component(&options.project),
        work_item_id
    )
}

pub fn create_work_item_url(options: &AzureDevOpsOptions, work_item_type: &str) -> String {
    format!(
        "{}/{}/_apis/wit/workitems/${}?api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        encode_component(work_item_type),
        api_version(options)
    )
}

pub fn pull_requests_url(options: &AzureDevOpsOptions, repository: &str) -> String {
    format!(
        "{}/{}/_apis/git/repositories/{}/pullrequests?api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        encode_component(repository),
        api_version(options)
    )
}

pub fn pull_request_web_url(
    options: &AzureDevOpsOptions,
    repository: &str,
    pull_request_id: i64,
) -> String {
    format!(
        "{}/{}/_git/{}/pullrequest/{}",
        options.organization.trim_end_matches('/'),
        encode_component(&options.project),
        encode_component(repository),
        pull_request_id
    )
}

pub fn active_pull_requests_url(
    options: &AzureDevOpsOptions,
    repository: &str,
    source_ref: &str,
) -> String {
    format!(
        "{}/{}/_apis/git/repositories/{}/pullrequests?searchCriteria.status=active&searchCriteria.sourceRefName={}&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        encode_component(repository),
        encode_component(source_ref),
        api_version(options)
    )
}

pub fn active_pull_requests_for_repository_url(
    options: &AzureDevOpsOptions,
    repository: &str,
) -> String {
    format!(
        "{}/{}/_apis/git/repositories/{}/pullrequests?searchCriteria.status=active&api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        encode_component(repository),
        api_version(options)
    )
}

pub fn pull_request_work_items_url(
    options: &AzureDevOpsOptions,
    repository: &str,
    pull_request_id: i64,
) -> String {
    format!(
        "{}/{}/_apis/git/repositories/{}/pullRequests/{}/workitems?api-version={}",
        options.organization.trim_end_matches('/'),
        options.project,
        encode_component(repository),
        pull_request_id,
        api_version(options)
    )
}

pub(crate) fn organization_name(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('/');
    trimmed
        .rsplit('/')
        .next()
        .filter(|segment| !segment.trim().is_empty())
        .unwrap_or(trimmed)
        .to_owned()
}

fn api_version(options: &AzureDevOpsOptions) -> &str {
    if options.api_version.trim().is_empty() {
        DEFAULT_API_VERSION
    } else {
        &options.api_version
    }
}

fn encode_component(value: &str) -> String {
    value.replace(' ', "%20").replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pull_request_web_url_targets_azure_devops_page_not_api_json() {
        let options = AzureDevOpsOptions {
            organization: "https://dev.azure.com/acme/".into(),
            project: "Hommage Agence".into(),
            api_version: "7.1".into(),
        };

        let url = pull_request_web_url(&options, "front app", 55264);

        assert_eq!(
            url,
            "https://dev.azure.com/acme/Hommage%20Agence/_git/front%20app/pullrequest/55264"
        );
        assert!(!url.contains("_apis"));
    }
}
