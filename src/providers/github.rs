use std::{convert::From, fmt};
use regex::Regex;

use chrono::{DateTime, Utc};
use reqwest;
use serde::{Deserialize, Serialize};
use serde_json::error::Error as JsonError;

const API_BASE_URL: &str = "https://api.github.com";
const PER_PAGE: usize = 100;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub struct Error {
    reason: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.reason)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

impl From<JsonError> for Error {
    fn from(err: JsonError) -> Self {
        Error {
            reason: err.to_string(),
        }
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error {
            reason: err.to_string(),
        }
    }
}

pub struct GitHub {
    token: String,
    client: reqwest::Client,
    repos: Vec<Repo>,
    projects: Vec<Project>,
}

struct Header {
    key: String,
    value: String,
}

#[derive(Debug, Eq, PartialEq)]
struct Repo {
    owner: String,
    repo: String,
}

impl From<String> for Repo {
    fn from(r: String) -> Self {
        let parsed = r.split("/").map(Into::into).collect::<Vec<String>>();
        Repo {
            owner: parsed[0].to_owned(),
            repo: parsed[1].to_owned(),
        }
    }
}


#[derive(Debug, Eq, PartialEq)]
struct Project {
    owner: String,
    repo: String,
    number: i32,
}

impl From<String> for Project {
    fn from(r: String) -> Self {
        let re = Regex::new(r"https://github.com/([\w-]+)/([\w-]+)/projects/(\d+)").unwrap();
        let mat = re.captures(&r[..]);
        if let Some(m) = mat {
            Project {
                owner: m.get(1).unwrap().as_str().to_owned(),
                repo: m.get(2).unwrap().as_str().to_owned(),
                number: m.get(3).unwrap().as_str().parse::<i32>().unwrap(),
            }
        } else {
            Project {
                owner: "".to_owned(),
                repo: "".to_owned(),
                number: 0,
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct User {
    login: String,
}

#[derive(Serialize, Deserialize)]
pub struct Pull {
    html_url: String,
}

#[derive(Serialize, Deserialize)]
pub struct Assignee {
    id: i64,
    login: String,
}

#[derive(Serialize, Deserialize)]
pub struct Label {
    id: i64,
    name: String,
    description: Option<String>,
}

#[derive(Serialize, Deserialize)]
pub struct Issue {
    number: i32,
    title: String,
    assignee: Option<Assignee>,
    #[serde(skip_deserializing)]
    owner: String,
    #[serde(skip_deserializing)]
    repo: String,
    pull_request: Option<Pull>,
    created_at: DateTime<Utc>,
    author_association: String,
    labels: Vec<Label>,
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "https://github.com/{}/{}/issues/{}",
            self.owner, self.repo, self.number
        )
    }
}

#[derive(Serialize, Deserialize)]
pub struct Comment {
    html_url: String,
    author_association: String,
}

impl GitHub {
    pub fn new(token: String, repos: Vec<String>, projects: Vec<String>) -> Self {
        let mut auth_header = "token ".to_owned();
        auth_header.push_str(&token);
        let repos: Vec<Repo> = repos.into_iter().map(Into::into).collect();
        let projects = projects.into_iter()
            .map(Into::into)
            .filter(|p: &Project| !&repos.contains(&Repo{owner: p.owner.to_owned(), repo: p.repo.to_owned()})).collect();
        GitHub {
            token: auth_header,
            client: reqwest::Client::new(),
            repos,
            projects,
        }
    }

    async fn request(&self, url: &str, headers: Vec<Header>) -> Result<String> {
        let mut req = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, "pingbot")
            .header(reqwest::header::AUTHORIZATION, &self.token[..]);
        for header in headers {
            req = req.header(&header.key[..], &header.value[..]);
        }
        let res = req.send().await?.text().await?;
        Ok(res)
    }

    pub async fn get_user_result(&self) -> Result<String> {
        let url = format!("{}/user", API_BASE_URL);
        let res = self.request(&url[..], vec![]).await?;
        let u: User = serde_json::from_str(&res[..])?;
        Ok(u.login.to_owned())
    }

    pub async fn get_opened_issues(&self, raw: Vec<String>) -> Result<Vec<Issue>> {
        let now = Utc::now();
        let repos = parse_repos(raw);
        let mut opened_all = vec![];
        for repo in repos {
            println!("process {}/{}", repo.owner, repo.repo);
            let issues = self.get_opened_issues_by_repo(&repo).await?;
            opened_all.extend(issues);
        }

        let opened_issues: Vec<Issue> = opened_all
            .into_iter()
            .filter(|issue| {
                if now.signed_duration_since(issue.created_at).num_hours() > 3 * 24 {
                    return false;
                }
                // if self.if_filter_by_label(&issue) {
                //     return false;
                // }
                issue.pull_request.is_none() && issue.assignee.is_none() // && !if_member(&issue.author_association)
            })
            .collect();

        let mut no_comment_issue = Vec::<Issue>::new();
        for issue in opened_issues {
            let comment_num = self.get_comments_by_issue(&issue).await?;
            if comment_num == 0 {
                no_comment_issue.push(issue);
            }
        }

        Ok(no_comment_issue)
    }

    async fn get_opened_issues_by_repo(&self, repo: &Repo) -> Result<Vec<Issue>> {
        let mut all = Vec::<Issue>::new();
        let mut page = 0;

        while all.len() == page * PER_PAGE {
            page += 1;
            let url = format!(
                "{}/repos/{}/{}/issues?page={}&per_page={}",
                API_BASE_URL, repo.owner, repo.repo, page, PER_PAGE
            );
            let headers = vec![Header {
                key: "Accept".to_owned(),
                value: "application/vnd.github.machine-man-preview".to_owned(),
            }];
            let res = self.request(&url[..], headers).await?;
            let batch: Vec<Issue> = serde_json::from_str(&res[..])?;
            all.extend(batch);
        }
        println!("all ok");

        Ok(all
            .into_iter()
            .map(|mut issue| {
                issue.owner = repo.owner.to_owned();
                issue.repo = repo.repo.to_owned();
                issue
            })
            .collect())
    }

    async fn get_comments_by_issue(&self, issue: &Issue) -> Result<usize> {
        let url = format!(
            "{}/repos/{}/{}/issues/{}/comments?per_page={}",
            API_BASE_URL, issue.owner, issue.repo, issue.number, PER_PAGE
        );
        let res = self.request(&url[..], vec![]).await?;
        let comments: Vec<Comment> = serde_json::from_str(&res[..])?;
        let member_comments: Vec<Comment> = comments
            .into_iter()
            .filter(|comment| if_member(&comment.author_association))
            .collect();
        Ok(member_comments.len())
    }

    // fn if_filter_by_label(&self, issue: &Issue) -> bool {
    //     for label in &issue.labels {
    //         let lower_label = label.name.to_lowercase();
    //         if self.filter_labels.contains(&lower_label) {
    //             return true;
    //         }
    //     }
    //     false
    // }
}

fn parse_repos(raw: Vec<String>) -> Vec<Repo> {
    raw.into_iter().map(Into::into).collect()
}

fn if_member(relation: &String) -> bool {
    relation == "OWNER"
        || relation == "COLLABORATOR"
        || relation == "MEMBER"
        || relation == "CONTRIBUTOR"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_client() -> GitHub {
        let repos = vec!["pingcap/parser".to_owned(), "you06/issues-watcher".to_owned()];
        let projects = vec![
            "https://github.com/pingcap/parser/projects/1".to_owned(),
            "https://github.com/pingcap/tidb/projects/40".to_owned(),
        ];
        GitHub::new("".to_owned(), repos, projects)
    }

    #[allow(dead_code)]
    fn new_issue_with_labels(labels: Vec<String>) -> Issue {
        Issue {
            number: 0,
            title: "title".to_owned(),
            assignee: None,
            owner: "".to_owned(),
            repo: "".to_owned(),
            pull_request: None,
            created_at: Utc::now(),
            author_association: "".to_owned(),
            labels: labels
                .into_iter()
                .map(|name| Label {
                    id: 0,
                    name: name,
                    description: Some("".to_owned()),
                })
                .collect(),
        }
    }
    
    #[test]
    fn create_client() {
        let client = new_client();
        assert_eq!(client.repos, vec![
            Repo{owner: "pingcap".to_owned(), repo: "parser".to_owned()},
            Repo{owner: "you06".to_owned(), repo: "issues-watcher".to_owned()},
        ]);
        assert_eq!(client.projects, vec![
            Project{owner: "pingcap".to_owned(), repo: "tidb".to_owned(), number: 40},
        ]);
    }
}
