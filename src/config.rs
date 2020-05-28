use std::{fs::read_to_string, io::Error};

use serde::Deserialize;
use toml;

#[derive(Deserialize)]
pub struct Config {
    #[serde(rename = "slack-token")]
    pub slack_token: String,
    #[serde(rename = "slack-channel")]
    pub slack_channel: String,

    #[serde(rename = "github-token")]
    pub github_token: String,
    #[serde(default = "default_github_data")]
    #[serde(rename = "github-data")]
    pub github_data: String,
    #[serde(default)]
    #[serde(rename = "repos")]
    pub repos: Vec<String>,
    #[serde(default)]
    #[serde(rename = "projects")]
    pub projects: Vec<String>,
}

fn default_github_data() -> String {
    "~/.issues-watcher".to_owned()
}

impl Config {
    pub fn new(filename: String) -> Result<Self, Error> {
        let contents = read_to_string(filename)?;
        let config: Config = toml::from_str(&contents[..]).unwrap();
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_config() -> Result<Config, Error> {
        Config::new("config.example.toml".to_owned())
    }

    #[test]
    fn read_config() {
        let config = new_config().unwrap();
        // slack
        assert_eq!(config.slack_token, "slack-token");
        assert_eq!(config.slack_channel, "slack-channel");
        // github
        assert_eq!(config.github_token, "github-token");
        assert_eq!(config.github_data, "~/.issues-watcher");
        assert_eq!(config.repos, vec!["pingcap/parser"]);
        assert_eq!(config.projects, vec!["https://github.com/pingcap/tidb/projects/40"]);
    }
}
