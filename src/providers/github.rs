use regex::Regex;
use std::{convert::From, fmt, collections::HashMap};

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

impl From<&str> for Error {
    fn from(err: &str) -> Self {
        Error {
            reason: err.to_string(),
        }
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

#[derive(Debug)]
pub struct GitHub {
    token: String,
    client: reqwest::Client,
    repos: Vec<Repo>,
    projects: Vec<Project>,
    time: DateTime<Utc>,
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

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Project {
    owner: String,
    repo: String,
    number: i32,
    id: Option<i64>,
}

#[derive(Serialize, Deserialize)]
struct GitHubProject {
    id: i64,
    number: i32
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
                id: None,
            }
        } else {
            Project {
                owner: "".to_owned(),
                repo: "".to_owned(),
                number: 0,
                id: None,
            }
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct User {
    login: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Pull {
    html_url: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Assignee {
    id: i64,
    login: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Label {
    id: i64,
    name: String,
    description: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
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

#[derive(Debug)]
pub struct RepoIssues<'a> {
    repo: &'a Repo,
    issues: Vec<Issue>,
}

#[derive(Debug)]
pub struct ProjectIssues<'a> {
    project: &'a Project,
    columns: Vec<Column>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Column {
    id: i64,
    name: String,
    #[serde(skip_deserializing)]
    cards: Vec<Card>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Card {

}

#[derive(Debug)]
pub struct Snapshot<'a> {
    time: &'a DateTime<Utc>,
    repo_issues: Vec<RepoIssues<'a>>,
    project_issues: Vec<ProjectIssues<'a>>,
}

impl GitHub {
    pub fn new(token: String, repos: Vec<String>, projects: Vec<String>) -> Self {
        let mut auth_header = "token ".to_owned();
        auth_header.push_str(&token);
        let repos: Vec<Repo> = repos.into_iter().map(Into::into).collect();
        let projects = projects
            .into_iter()
            .map(Into::into)
            .filter(|p: &Project| {
                !&repos.contains(&Repo {
                    owner: p.owner.to_owned(),
                    repo: p.repo.to_owned(),
                })
            })
            .collect();
        GitHub {
            token: auth_header,
            client: reqwest::Client::new(),
            repos,
            projects,
            time: Utc::now(),
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

    // pub async fn get_issues(&self) -> Result<Vec<Issue>> {
    //     let mut opened_all = vec![];
    //     for repo in self.repos.iter() {
    //         println!("process {}/{}", repo.owner, repo.repo);
    //         let issues = self.get_opened_issues_by_repo(&repo).await?;
    //         opened_all.extend(issues);
    //     }

    //     let opened_issues: Vec<Issue> = opened_all
    //         .into_iter()
    //         .filter(|issue| issue.pull_request.is_none())
    //         .collect();

    //     Ok(opened_issues)
    // }

    async fn get_opened_issues_by_repo<'a> (&self, repo: &'a Repo) -> Result<RepoIssues<'a>> {
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

        let opened_all = all
            .into_iter()
            .map(|mut issue| {
                issue.owner = repo.owner.to_owned();
                issue.repo = repo.repo.to_owned();
                issue
            })
            .collect();

        Ok(RepoIssues{
            repo: repo,
            issues: opened_all,
        })
    }

    // async fn get_comments_by_issue(&self, issue: &Issue) -> Result<usize> {
    //     let url = format!(
    //         "{}/repos/{}/{}/issues/{}/comments?per_page={}",
    //         API_BASE_URL, issue.owner, issue.repo, issue.number, PER_PAGE
    //     );
    //     let res = self.request(&url[..], vec![]).await?;
    //     let comments: Vec<Comment> = serde_json::from_str(&res[..])?;
    //     let member_comments: Vec<Comment> = comments
    //         .into_iter()
    //         .filter(|comment| if_member(&comment.author_association))
    //         .collect();
    //     Ok(member_comments.len())
    // }

    async fn get_opened_issues<'a> (&'a self) -> Result<Vec<RepoIssues<'a>>> {
        let mut repos: Vec<RepoIssues> = Vec::new();
        for repo in &self.repos {
            let repo_issues = self.get_opened_issues_by_repo(repo).await?;
            repos.push(repo_issues);
        }
        Ok(repos)
    }

    pub async fn get_projects_id(&mut self) -> Result<()> {
        let mut number2id = HashMap::new();
        for project in &self.projects {
            if let None = project.id {
                let mut page = 0;

                'outer: loop {
                    page += 1;
                    let url = format!("{}/repos/{}/{}/projects?page={}&per_page={}", API_BASE_URL, project.owner, project.repo, page, PER_PAGE);
                    let res = self.request(&url[..], vec![
                        Header{
                            key: "Accept".to_owned(),
                            value: "application/vnd.github.inertia-preview+json".to_owned(),
                        }
                    ]).await?;
                    let ps: Vec<GitHubProject> = serde_json::from_str(&res[..])?;
                    for p in &ps {
                        if p.number == project.number {
                            number2id.insert(p.number, p.id);
                            break 'outer;
                        }
                    }
                    if ps.len() < PER_PAGE {
                        return Err("project not found".into())
                    }
                }
            }
        }
        for project in &mut self.projects {
            if let None = project.id {
                match number2id.get(&project.number) {
                    Some(&id) => project.id = Some(id),
                    None => return Err("project not found".into())
                }
            }
        }
        Ok(())
    }

    pub fn get_projects(&self) -> Vec<Project> {
        self.projects.clone()
    }

    async fn get_cards_by_column(&self, column: &Column) -> Result<Card> {
        Err("implement me".into())
    }

    async fn get_cards(&self, column_id: i64) -> Result<Vec<Card>> {
        let mut all = vec![];
        let mut page = 0;
        while all.len() == page * PER_PAGE {
            page += 1;
            let url = format!("{}/projects/columns/{}/cards?page={}&per_page={}", API_BASE_URL, column_id, page, PER_PAGE);
            let res = self.request(&url[..], vec![
                Header{
                    key: "Accept".to_owned(),
                    value: "application/vnd.github.inertia-preview+json".to_owned(),
                }
            ]).await?;
            let batch: Vec<Card> = serde_json::from_str(&res[..])?;
            all.extend(batch);
        }
        Ok(all)
    }

    async fn get_columns(&self, project: &Project) -> Result<Vec<Column>> {
        if let Some(project_id) = project.id {
            let url = format!("{}/projects/{}/columns?per_page={}", API_BASE_URL, project_id, PER_PAGE);
            let res = self.request(&url[..], vec![
                Header{
                    key: "Accept".to_owned(),
                    value: "application/vnd.github.inertia-preview+json".to_owned(),
                }
            ]).await?;
            let mut columns: Vec<Column> = serde_json::from_str(&res[..])?;
            for column in columns.iter_mut() {
                (*column).cards = self.get_cards(column.id).await?;
            }
            Ok(columns)
        } else {
            Err("project id is none".into())
        }
    }

    async fn get_project<'a> (&'a self, project: &'a Project) -> Result<ProjectIssues<'a>> {
        let columns = self.get_columns(project).await?;

        Ok(ProjectIssues{
            project: project,
            columns: columns,
        })
    }

    async fn get_projects_snapshot<'a> (&'a self) -> Result<Vec<ProjectIssues<'a>>> {
        let mut projects: Vec<ProjectIssues> = Vec::new();
        for project in &self.projects {
            let project_issues = self.get_project(project).await?;
            projects.push(project_issues);
        }
        Ok(projects)
    }

    pub async fn get_snapshot<'a> (&'a self) -> Result<Snapshot<'a>> {
        let repo_issues = self.get_opened_issues().await?;
        let projects = self.get_projects_snapshot().await?;
        Ok(Snapshot{
            time: &self.time,
            repo_issues: repo_issues,
            project_issues: projects,
        })
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
        let repos = vec![
            "pingcap/parser".to_owned(),
            "you06/issues-watcher".to_owned(),
        ];
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
        assert_eq!(
            client.repos,
            vec![
                Repo {
                    owner: "pingcap".to_owned(),
                    repo: "parser".to_owned()
                },
                Repo {
                    owner: "you06".to_owned(),
                    repo: "issues-watcher".to_owned()
                },
            ]
        );
        assert_eq!(
            client.projects,
            vec![Project {
                owner: "pingcap".to_owned(),
                repo: "tidb".to_owned(),
                number: 40,
                id: None,
            },]
        );
    }
}
