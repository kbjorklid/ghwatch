use crate::domain::pr::PullRequest;

#[must_use]
pub fn filter_prs(prs: &[PullRequest], query: &str) -> Vec<PullRequest> {
    if query.is_empty() {
        return prs.to_vec();
    }

    let query = query.to_lowercase();
    prs.iter()
        .filter(|pr| {
            pr.title.to_lowercase().contains(&query)
                || pr.author.to_lowercase().contains(&query)
                || pr.repo.to_lowercase().contains(&query)
                || format!("#{}", pr.number).contains(&query)
        })
        .cloned()
        .collect()
}
