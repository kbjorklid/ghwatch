use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct RawPullRequest {
    pub id: String,
    pub number: u32,
    pub title: String,
    pub author: RawAuthor,
    pub repository: Option<RawRepository>,
    #[serde(rename = "headRepository")]
    pub head_repository: Option<RawRepository>,
    pub state: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub body: String,
    #[serde(rename = "commentsCount")]
    pub comments_count_search: Option<u32>,
    pub comments: Option<Vec<RawComment>>,
    pub additions: Option<u32>,
    pub deletions: Option<u32>,
    #[serde(rename = "reviewDecision")]
    pub review_decision: Option<String>,
    #[serde(rename = "statusCheckRollup")]
    pub status_check_rollup: Option<RawStatusCheckRollup>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RawAuthor {
    pub login: String,
}

#[derive(Debug, Deserialize)]
pub struct RawRepository {
    #[serde(rename = "nameWithOwner")]
    pub name_with_owner: String,
}

#[derive(Debug, Deserialize)]
pub struct RawComment {
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct RawStatusCheckRollup {
    pub state: String,
}

#[derive(Debug, Deserialize)]
pub struct RawCheckRun {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct RawTimelineEvent {
    pub id: Option<String>,
    #[serde(rename = "__typename")]
    pub typename: String,
    pub actor: Option<RawAuthor>,
    #[serde(rename = "createdAt")]
    pub created_at: Option<String>,
    // Add more fields as needed for different event types
}
