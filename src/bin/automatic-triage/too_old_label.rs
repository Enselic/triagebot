use chrono::Duration;
use triagebot::github::{GithubClient, Repository};

use cynic::QueryBuilder;
use github_graphql::queries::{self, TooOldLabelIssue};

pub async fn issues_with_minimum_label_age(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    minimum_label_age: Duration,
    client: &GithubClient,
) -> anyhow::Result<Vec<TooOldLabelIssue>> {
    let mut issues: Vec<queries::TooOldLabelIssue> = vec![];

    let mut args = queries::TooOldLabelArguments {
        repository_owner: repository_owner.to_owned(),
        repository_name: repository_name.to_owned(),
        label: label.to_owned(),
        after: None,
    };

    let now = chrono::Utc::now();

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

        issues.extend(repository.issues.nodes.into_iter().filter());

        let page_info = repository.issues.page_info;
        if !page_info.has_next_page || page_info.end_cursor.is_none() {
            break;
        }
        args.after = page_info.end_cursor;
    }

    for issue in &issues {
        println!("issue: {:?}", issue);
    }

    Ok(issues)
}

fn has_too_old_comment(issue: TooOldLabelIssue) {
    let now = chrono::Utc::now();
    let last_comment_at = issue.comments.nodes.first().map(|c|c.created_at).unwrap_or_else(|| issue.created_at);
    let comment_age = now - last_comment.created_at;
    if comment_age > chrono::Duration::days(30) {
        println!("issue: {:?}", issue);
    }
}
