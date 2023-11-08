use chrono::{DateTime, Duration, Utc};
use tracing::warn;
use triagebot::github::{GithubClient, Repository};

use cynic::QueryBuilder;
use github_graphql::queries::{self, OldLabelCandidateIssue};

pub async fn triage_old_label(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    minimum_age: Duration,
    client: &GithubClient,
) -> anyhow::Result<Vec<OldLabelCandidateIssue>> {
    let now = chrono::Utc::now();

    let candidates = issues_with_label(repository_owner, repository_name, label, client)
        .await?
        .into_iter()
        .filter(|issue| filter_last_comment_age(issue, minimum_age, &now))
        .collect::<Vec<_>>();

    Ok(vec![])
}

fn filter_last_comment_age(
    issue: &OldLabelCandidateIssue,
    minimum_age: Duration,
    now: &DateTime<Utc>,
) -> bool {
    let now = chrono::Utc::now();

    let last_comment_at = issue
        .comments
        .nodes
        .last()
        .map(|c| c.created_at)
        .unwrap_or_else(|| issue.created_at);
    let comment_age = now - last_comment_at;
    if comment_age > minimum_age {
        true
    } else {
        println!(
            "{} commented less than {} months ago, namely {} months ago. No action.",
            issue.url.0,
            minimum_age.num_days() / 30,
            comment_age.num_days() / 30,
        );
        false
    }
}

pub fn label_age(issue: &IssueWithTimelineItems) -> anyhow::Result<(Duration, Duration)> {
    let mut last_labeled_at = None;
    let mut last_commented_at = None;

    for timeline_item in &issue.timeline_items {
        if let TimelineItem::LabeledEvent {
            label: Label { name },
            created_at,
        } = timeline_item
        {
            if name == E_NEEDS_MCVE {
                last_labeled_at = Some(*created_at);
            }
        }
        if let TimelineItem::IssueComment { created_at, .. } = timeline_item {
            last_commented_at = Some(*created_at);
        }
    }

    let now = chrono::Utc::now();
    let label_age =
        now.signed_duration_since(last_labeled_at.expect("only labeled issues queried for"));
    let last_comment_age = now.signed_duration_since(last_commented_at.unwrap_or(issue.created_at));
    Ok((label_age, last_comment_age))
}

pub async fn issues_with_label(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    client: &GithubClient,
) -> anyhow::Result<Vec<OldLabelCandidateIssue>> {
    let mut issues: Vec<queries::OldLabelCandidateIssue> = vec![];

    let mut args = queries::OldLabelArguments {
        repository_owner: repository_owner.to_owned(),
        repository_name: repository_name.to_owned(),
        label: label.to_owned(),
        after: None,
    };

    loop {
        let query = queries::OldLabelIssuesQuery::build(args.clone());
        let req = client.post(Repository::GITHUB_GRAPHQL_API_URL);
        let req = req.json(&query);

        warn!("Running query (rate limit affected)");
        let data: cynic::GraphQlResponse<queries::OldLabelIssuesQuery> = client.json(req).await?;

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

    Ok(issues)
}

// fn has_too_old_comment(issue: TooOldLabelIssue) {
//     let now = chrono::Utc::now();
//     let last_comment_at = issue.comments.nodes.first().map(|c|c.created_at).unwrap_or_else(|| issue.created_at);
//     let comment_age = now - last_comment.created_at;
//     if comment_age > chrono::Duration::days(30) {
//         println!("issue: {:?}", issue);
//     }
// }
