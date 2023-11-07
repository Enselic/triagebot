use chrono::Duration;
use reqwest::Client;
use triagebot::github::{GithubClient, Repository};

use cynic::QueryBuilder;
use github_graphql::queries::{self, TooOldLabelIssue};



pub struct TooOldLabel {
    name: String,
    age_considered_too_old: Duration,
}

async fn issues_with_minimal_label_age(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    minimal_label_age: Duration,
    client: &GithubClient,
) -> anyhow::Result<Vec<TooOldLabelIssue>> {

    let mut issues: Vec<queries::TooOldLabelIssue> = vec![];

    let mut args = queries::TooOldLabelArguments {
        repository_owner: repository_owner.to_owned(),
        repository_name: repository_name.to_owned(),
        label: label.to_owned(),
        after: None,
    };

    loop {
        let query = queries::TooOldLabelIssuesQuery::build(args.clone());
        let req = client.post(Repository::GITHUB_GRAPHQL_API_URL);
        let req = req.json(&query);

        let data: cynic::GraphQlResponse<queries::TooOldLabelIssuesQuery> =
            client.json(req).await?;
        if let Some(errors) = data.errors {
            anyhow::bail!("There were graphql errors. {:?}", errors);
        }
        let repository = data
            .data
            .ok_or_else(|| anyhow::anyhow!("No data returned."))?
            .repository
            .ok_or_else(|| anyhow::anyhow!("No repository."))?;

        issues.extend(repository.issues.nodes);

        let page_info = repository.issues.page_info;
        if !page_info.has_next_page || page_info.end_cursor.is_none() {
            break;
        }
        args.after = page_info.end_cursor;
    }

    for issue in issues {
        println!("issue: {:?}", issue);
    }

    Ok(issues)
}
