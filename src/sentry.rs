use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub level: String,
    pub culprit: String,
    #[serde(rename = "lastSeen")]
    pub last_seen: String,
    pub count: u32,
    #[serde(rename = "userCount")]
    pub user_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Project {
    pub slug: String,
    pub name: String,
}

#[derive(Clone)]
pub struct SentryClient {
    client: Client,
    base_url: String,
    auth_token: Option<String>,
}

impl SentryClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            client: Client::new(),
            base_url: "https://sentry.io/api/0".to_string(),
            auth_token: None,
        })
    }

    pub fn login(&mut self, auth_token: String) -> Result<()> {
        self.auth_token = Some(auth_token);
        Ok(())
    }

    fn get_headers(&self) -> Result<HeaderMap> {
        let auth_token = self.auth_token.as_ref()
            .context("Not authenticated. Please set the auth token first.")?;

        let mut headers = HeaderMap::new();
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", auth_token))
                .context("Invalid auth token")?,
        );
        Ok(headers)
    }

    pub fn list_projects(&self, org_slug: &str) -> Result<Vec<Project>> {
        let url = format!(
            "{}/organizations/{}/projects/",
            self.base_url, org_slug
        );

        let response = self.client
            .get(&url)
            .headers(self.get_headers()?)
            .send()
            .context("Failed to send request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "API request failed: {} - {}",
                response.status(),
                response.text()?
            ));
        }

        response.json::<Vec<Project>>()
            .context("Failed to parse response")
    }

    pub fn list_issues(&self, org_slug: &str, project_slug: &str) -> Result<Vec<Issue>> {
        let url = format!(
            "{}/projects/{}/{}/issues/?statsPeriod=14d&query=is:unresolved&sort=date",
            self.base_url, org_slug, project_slug
        );

        let response = self.client
            .get(&url)
            .headers(self.get_headers()?)
            .send()
            .context("Failed to send request")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "API request failed: {} - {}",
                response.status(),
                response.text()?
            ));
        }

        response.json::<Vec<Issue>>()
            .context("Failed to parse response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_client_creation() {
        let client = SentryClient::new();
        assert!(client.is_ok());
    }

    #[test]
    fn test_login() {
        let mut client = SentryClient::new().unwrap();
        client.login("test-token".to_string()).unwrap();
        assert!(client.auth_token.is_some());
    }
} 