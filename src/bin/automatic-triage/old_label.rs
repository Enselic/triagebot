use chrono::{DateTime, Duration, Utc};
use tracing::{debug, info};
use triagebot::github::{GithubClient, Repository};

use cynic::QueryBuilder;
use github_graphql::queries::*;

struct AnalyzedIssue {
    number: i32,
    url: String,
    time_until_close: Duration,
}

pub async fn triage_old_label(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    exclude_labels_containing: &str,
    minimum_age: Duration,
    client: &GithubClient,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now();

    let mut issues = issues_with_label(repository_owner, repository_name, label, client)
        .await?
        .into_iter()
        .filter(|issue| filter_excluded_labels(issue, exclude_labels_containing))
        .map(|issue| {
            // If an issue is actively discussed, there is no limit on the age of the
            // label. We don't want to close issues that people are actively commenting on.
            // So require the last comment to also be old.
            let last_comment_age = last_comment_age(&issue, &now);

            let label_age = label_age(&issue, label, &now);

            AnalyzedIssue {
                number: issue.number,
                url: issue.url.0,
                time_until_close: minimum_age - std::cmp::min(label_age, last_comment_age),
            }
        })
        .collect::<Vec<_>>();

    issues.sort_by_key(|issue| std::cmp::Reverse(issue.time_until_close));

    for issue in issues {
        if issue.time_until_close.num_days() > 0 {
            println!(
                "{} will be closed after {} months",
                issue.url,
                issue.time_until_close.num_days() / 30
            );
        } else {
            println!(
                "{} will be closed now (FIXME: Actually implement closing)",
                issue.url,
            );
            close_issue(issue.number, client).await;
        }
    }

    Ok(())
}

pub async fn issues_with_label(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    client: &GithubClient,
) -> anyhow::Result<Vec<OldLabelCandidateIssue>> {
    let mut issues: Vec<OldLabelCandidateIssue> = vec![];

    let mut args = OldLabelArguments {
        repository_owner: repository_owner.to_owned(),
        repository_name: repository_name.to_owned(),
        label: label.to_owned(),
        after: None,
    };

    let mut max_iterations_left = 100;
    loop {
        max_iterations_left -= 1;
        if max_iterations_left < 0 {
            anyhow::bail!("Bailing to avoid rate limit depletion. The code might be buggy.");
        }

        let query = OldLabelIssuesQuery::build(args.clone());
        let req = client.post(Repository::GITHUB_GRAPHQL_API_URL);
        let req = req.json(&query);

        info!("GitHub GraphQL API endpoint request (affects rate limit)");
        let data: cynic::GraphQlResponse<OldLabelIssuesQuery> = client.json(req).await?;

        if let Some(errors) = data.errors {
            anyhow::bail!("There were graphql errors. {:?}", errors);
        }

        let repository = data
            .data
            .ok_or_else(|| anyhow::anyhow!("No data returned."))?
            .repository
            .ok_or_else(|| anyhow::anyhow!("No repository."))?;

        issues.extend(repository.issues.nodes);

        debug!(
            "Now have {} issues of {}",
            issues.len(),
            repository.issues.total_count
        );

        let page_info = repository.issues.page_info;
        if !page_info.has_next_page || page_info.end_cursor.is_none() {
            break;
        }
        args.after = page_info.end_cursor;
    }

    Ok(issues)
}

fn filter_excluded_labels(issue: &OldLabelCandidateIssue, exclude_labels_containing: &str) -> bool {
    !issue.labels.as_ref().unwrap().nodes.iter().any(|label| {
        label
            .name
            .to_lowercase()
            .contains(exclude_labels_containing)
    })
}

fn last_comment_age(issue: &OldLabelCandidateIssue, now: &DateTime<Utc>) -> Duration {
    let last_comment_at = issue
        .comments
        .nodes
        .last()
        .map(|c| c.created_at)
        .unwrap_or_else(|| issue.created_at);

    *now - last_comment_at
}

pub fn label_age(issue: &OldLabelCandidateIssue, label: &str, now: &DateTime<Utc>) -> Duration {
    let timeline_items = &issue.timeline_items.as_ref().unwrap();

    if timeline_items.page_info.has_next_page {
        eprintln!(
            "{} has more than 250 `LabeledEvent`s. We need to implement paging!",
            issue.url.0
        );
        return Duration::days(30 * 999999);
    }

    let mut last_labeled_at = None;

    // The way the GraphQL query is constructed guarantees that we see the
    // oldest event first, so we can simply iterate sequentially. And we don't
    // need to bother with UnlabeledEvent since in the query we require the
    // label to be present, so we know it has not been unlabeled in the last
    // event.
    for timeline_item in &timeline_items.nodes {
        if let IssueTimelineItems::LabeledEvent(LabeledEvent {
            label: Label { name },
            created_at,
        }) = timeline_item
        {
            if name == label {
                last_labeled_at = Some(created_at);
            }
        }
    }

    now.signed_duration_since(
        *last_labeled_at.expect("The GraphQL query only includes issues that has the label"),
    )
}

async fn close_issue(_number: i32, _client: &GithubClient) {
    // FIXME: Actually close the issue
    // FIXME: Report to "triagebot closed issues" topic in "t-release/triage" Zulip
}
