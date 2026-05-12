use crate::domain::pr::PullRequest;
use crate::config::{AppConfig, GroupMode};

pub struct PRList {
    prs: Vec<PullRequest>,
    selected_index: usize,
}

pub struct PrGroup<'a> {
    pub name: String,
    pub prs: Vec<&'a PullRequest>,
}

impl PRList {
    pub fn new(prs: Vec<PullRequest>) -> Self {
        Self {
            prs,
            selected_index: 0,
        }
    }

    pub fn items(&self) -> &[PullRequest] {
        &self.prs
    }

    pub fn items_mut(&mut self) -> &mut Vec<PullRequest> {
        &mut self.prs
    }

    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, index: usize) {
        if index < self.prs.len() {
            self.selected_index = index;
        } else if !self.prs.is_empty() {
            self.selected_index = self.prs.len() - 1;
        } else {
            self.selected_index = 0;
        }
    }

    pub fn select_next(&mut self) {
        if !self.prs.is_empty() && self.selected_index < self.prs.len() - 1 {
            self.selected_index += 1;
        }
    }

    pub fn select_prev(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }
    
    pub fn selected_pr(&self) -> Option<&PullRequest> {
        self.prs.get(self.selected_index)
    }

    pub fn remove_selected(&mut self) -> Option<PullRequest> {
        if self.selected_index < self.prs.len() {
            let pr = self.prs.remove(self.selected_index);
            if self.selected_index >= self.prs.len() && !self.prs.is_empty() {
                self.selected_index = self.prs.len() - 1;
            }
            Some(pr)
        } else {
            None
        }
    }

    pub fn insert_at_front(&mut self, pr: PullRequest) {
        self.prs.insert(0, pr);
    }

    pub fn set_prs(&mut self, prs: Vec<PullRequest>) {
        let old_id = self.selected_pr().map(|p| p.id.clone());
        self.prs = prs;
        
        // Try to maintain selection
        if let Some(id) = old_id {
            if let Some(new_idx) = self.prs.iter().position(|p| p.id == id) {
                self.selected_index = new_idx;
            } else if self.selected_index >= self.prs.len() && !self.prs.is_empty() {
                self.selected_index = self.prs.len() - 1;
            }
        } else if self.selected_index >= self.prs.len() && !self.prs.is_empty() {
            self.selected_index = self.prs.len() - 1;
        }
    }

    pub fn get_grouped_items<'a>(&'a self, config: &AppConfig) -> Vec<PrGroup<'a>> {
        get_grouped_items(&self.prs, config)
    }
}

pub fn get_grouped_items<'a>(prs: &'a [PullRequest], config: &AppConfig) -> Vec<PrGroup<'a>> {
    if config.group_by == GroupMode::None || prs.is_empty() {
        return vec![PrGroup { name: "All PRs".to_string(), prs: prs.iter().collect() }];
    }

    let mut groups = Vec::new();
    let mut current_name = String::new();
    let mut current_prs = Vec::new();

    for pr in prs {
        let name = match config.group_by {
            GroupMode::None => "All PRs".to_string(),
            GroupMode::Repo => pr.repo.clone(),
            GroupMode::Author => pr.author.clone(),
            GroupMode::Status => pr.status.to_string(),
            GroupMode::MyVsOther => {
                if pr.author == config.current_user {
                    "Mine".to_string()
                } else {
                    "Others".to_string()
                }
            }
        };

        if current_name.is_empty() {
            current_name = name.clone();
        }

        if name != current_name {
            groups.push(PrGroup { name: current_name.clone(), prs: current_prs });
            current_name = name;
            current_prs = Vec::new();
        }
        current_prs.push(pr);
    }

    if !current_prs.is_empty() {
        groups.push(PrGroup { name: current_name, prs: current_prs });
    }

    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::pr::{PRStatus, ReviewStatus, CIStatus};

    fn create_test_pr(id: &str) -> PullRequest {
        PullRequest {
            id: id.to_string(),
            number: 1,
            title: "Test".to_string(),
            author: "alice".to_string(),
            repo: "repo".to_string(),
            status: PRStatus::Open,
            created_at: "".to_string(),
            updated_at: "".to_string(),
            additions: 0,
            deletions: 0,
            review_status: ReviewStatus::Pending,
            comment_count: 0,
            unresolved_count: 0,
            total_resolvable_count: 0,
            conversational_count: 0,
            ci_status: CIStatus::Passing,
            head_ref: "".to_string(),
            body: "".to_string(),
            url: "".to_string(),
            requested_reviewers: vec![],
            reviewers: vec![],
            last_seen_at: None,
            last_seen_unresolved_count: 0,
            last_seen_total_resolvable_count: 0,
            last_seen_conversational_count: 0,
        }
    }

    #[test]
    fn test_navigation() {
        let mut list = PRList::new(vec![create_test_pr("1"), create_test_pr("2")]);
        assert_eq!(list.selected_index(), 0);
        list.select_next();
        assert_eq!(list.selected_index(), 1);
        list.select_next();
        assert_eq!(list.selected_index(), 1);
        list.select_prev();
        assert_eq!(list.selected_index(), 0);
    }

    #[test]
    fn test_set_prs_maintains_selection() {
        let mut list = PRList::new(vec![create_test_pr("1"), create_test_pr("2")]);
        list.select_next();
        assert_eq!(list.selected_index(), 1);
        
        list.set_prs(vec![create_test_pr("2"), create_test_pr("1")]);
        assert_eq!(list.selected_index(), 0); // "2" moved to index 0
    }
}
