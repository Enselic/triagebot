use chrono::{DateTime, Duration, Utc};
use tracing::warn;
use triagebot::github::{GithubClient, Repository};

use cynic::QueryBuilder;
use github_graphql::queries::*;

pub async fn triage_old_label(
    repository_owner: &str,
    repository_name: &str,
    label: &str,
    minimum_age: Duration,
    client: &GithubClient,
) {
    let now = chrono::Utc::now();

    let issues_to_close = issues_with_label(repository_owner, repository_name, label, client)
        .await
        .unwrap()
        .into_iter()
        .filter(|issue| filter_last_comment_age(issue, minimum_age, &now))
        .filter(|issue| filter_label_age(issue, label, minimum_age, &now))
        .collect::<Vec<_>>();

    // TODO: have no label that contains the word "triaged", e.g. AsyncAwait-Triaged.
    // , this is just a dry run. See https://rust-lang.zulipchat.com/#narrow/stream/242269-t-release.2Ftriage/topic/auto-close.20E-needs-mcve

    for issue in &issues_to_close {
        println!(
            "{} will be closed. TODO: Actually implement closing",
            issue.url.0
        );
        // FIXME: Actually close the issue
        // FIXME: Report the close to a Zulip topic called "triagebot closed issues" in the "t-release/triage" stream
    }
}

/// If an issue is actively discussed, there is no limit on the age of the
/// label. We don't want to close issues that people are actively commenting on.
/// So require the last comment to also be old.
///
/// We filter on comment age before label age so we don't have to unnecessarily
/// make paged queries on timeline events to get label history. If the last
/// comment is  young, the label age does not matter.
fn filter_last_comment_age(
    issue: &OldLabelCandidateIssue,
    minimum_age: Duration,
    now: &DateTime<Utc>,
) -> bool {
    let last_comment_at = issue
        .comments
        .nodes
        .last()
        .map(|c| c.created_at)
        .unwrap_or_else(|| issue.created_at);

    let last_comment_age = *now - last_comment_at;

    if last_comment_age > minimum_age {
        true
    } else {
        println!(
            "{} commented less than {} months ago, namely {} months ago. No action.",
            issue.url.0,
            minimum_age.num_days() / 30,
            last_comment_age.num_days() / 30,
        );
        false
    }
}

fn filter_label_age(
    issue: &OldLabelCandidateIssue,
    label: &str,
    minimum_age: Duration,
    now: &DateTime<Utc>,
) -> bool {
    let timeline_items = &issue.timeline_items.as_ref().unwrap();
    if timeline_items.page_info.has_next_page {
        eprintln!(
            "{} has more than 250 LabeledEvents. We need to implement paging!",
            issue.url.0
        );
        return false;
    }

    let label_age = label_age(&timeline_items.nodes, label, now);
    if label_age > minimum_age {
        true
    } else {
        println!(
            "{} labeled {} less than {} months ago, namely {} months ago. No action.",
            issue.url.0,
            label,
            minimum_age.num_days() / 30,
            label_age.num_days() / 30,
        );
        false
    }
}

pub fn label_age(
    timeline_items: &[IssueTimelineItems],
    label: &str,
    now: &DateTime<Utc>,
) -> Duration {
    let mut last_labeled_at = None;

    // The way the GraphQL query is constructed guarantees that we see the
    // oldest event first, so we can simply iterate sequentially. And we don't
    // need to bother with UnlabeledEvent since in the query we require the
    // label to be present, so we know it has not been unlabeled in the last
    // event.
    for timeline_item in timeline_items {
        if let IssueTimelineItems::LabeledEvent(LabeledEvent {
            label: Label { name },
            created_at,
        }) = timeline_item
        {
            if name == label {
                last_labeled_at = Some(*created_at);
            }
        }
    }

    now.signed_duration_since(last_labeled_at.expect("query ensures label exist"))
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

    loop {
        let query = OldLabelIssuesQuery::build(args.clone());
        let req = client.post(Repository::GITHUB_GRAPHQL_API_URL);
        let req = req.json(&query);

        warn!("Running query (rate limit affected)");
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

        let page_info = repository.issues.page_info;
        if !page_info.has_next_page || page_info.end_cursor.is_none() {
            break;
        }
        args.after = page_info.end_cursor;
    }

    Ok(issues)
}
