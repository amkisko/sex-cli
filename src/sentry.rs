use anyhow::{Context, Result};
use rand::{thread_rng, Rng};
use reqwest::blocking::Client;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use rpassword::prompt_password;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::{self, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use urlencoding;

const SENTRY_OAUTH_URL: &str = "https://sentry.io/oauth/authorize";
const REDIRECT_URI: &str = "http://localhost:8123/callback";

fn get_client_id() -> Result<String> {
    dotenvy::dotenv().ok(); // Load .env file if it exists
    env::var("SENTRY_CLIENT_ID").context("SENTRY_CLIENT_ID environment variable not set")
}

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
    pub platform: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(rename = "firstEvent")]
    pub first_event: Option<String>,
    #[serde(rename = "lastEvent")]
    pub last_event: Option<String>,
    pub stats: Option<ProjectStats>,
    pub id: Option<String>,
    pub isBookmarked: Option<bool>,
    pub isMember: Option<bool>,
    pub hasAccess: Option<bool>,
    pub teams: Option<Vec<Team>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProjectStats {
    #[serde(rename = "24h")]
    pub last_24h: Vec<(i64, i64)>,
    #[serde(rename = "30d")]
    pub last_30d: Vec<(i64, i64)>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Organization {
    pub slug: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Team {
    pub id: String,
    pub name: String,
    pub slug: String,
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
            base_url: Self::get_base_url(),
            auth_token: None,
        })
    }

    #[cfg(not(test))]
    fn get_base_url() -> String {
        "https://sentry.io/api/0".to_string()
    }

    #[cfg(test)]
    fn get_base_url() -> String {
        "http://localhost:1234".to_string()
    }

    pub fn login_with_prompt(&mut self) -> Result<()> {
        let token = prompt_password("Enter your Sentry auth token: ")
            .context("Failed to read auth token")?;
        self.login(token)
    }

    pub(crate) fn get_current_token(&self) -> Option<String> {
        self.auth_token.clone()
    }

    pub fn login(&mut self, auth_token: String) -> Result<()> {
        self.auth_token = Some(auth_token);
        Ok(())
    }

    pub fn list_organizations(&self) -> Result<Vec<Organization>> {
        let url = format!("{}/organizations/", self.base_url);

        let response = self
            .client
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

        response
            .json::<Vec<Organization>>()
            .context("Failed to parse response")
    }

    pub fn login_with_browser(&mut self) -> Result<Organization> {
        // Start local server to receive OAuth callback
        let listener = TcpListener::bind("127.0.0.1:8123")?;
        println!("Starting local server for OAuth callback...");

        // Generate OAuth URL with all required parameters
        let auth_url = format!(
            "{}?client_id={}&response_type=token&redirect_uri={}&scope={}&state={}",
            SENTRY_OAUTH_URL,
            get_client_id()?,
            REDIRECT_URI,
            "org:read project:read team:read member:read",
            Self::generate_state()
        );

        // Create a success page that extracts the token from URL fragment
        let success_page = r#"
            <html>
            <body>
                <h1>Waiting for authentication...</h1>
                <script>
                    function handleAuth() {
                        const hash = window.location.hash;
                        if (!hash) {
                            document.body.innerHTML = '<h1>Error</h1><p>No authentication data received. Please try again.</p>';
                            return;
                        }

                        // Remove the leading # and parse parameters
                        const params = new URLSearchParams(hash.substring(1));
                        const token = params.get('access_token');

                        if (!token) {
                            document.body.innerHTML = '<h1>Error</h1><p>No access token found. Please try again.</p>';
                            return;
                        }

                        // Send token back to the server by redirecting to /token endpoint
                        window.location.href = '/token?access_token=' + encodeURIComponent(token);
                    }

                    // Run the auth handler when the page loads
                    handleAuth();
                </script>
            </body>
            </html>
        "#;

        // Start background thread to handle browser callback
        let (tx, rx) = std::sync::mpsc::channel();
        let _handle = std::thread::spawn(move || {
            // Accept up to 2 connections (callback and token)
            for _ in 0..2 {
                if let Ok(mut stream) = listener.accept().map(|(s, _)| s) {
                    let mut buffer = [0; 1024];
                    if stream.read(&mut buffer).is_ok() {
                        let request = String::from_utf8_lossy(&buffer[..]);
                        // First request - serve the success page
                        if request.contains("GET /callback") {
                            let response = format!(
                                "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\n\r\n{}",
                                success_page.len(),
                                success_page
                            );
                            let _ = stream.write_all(response.as_bytes());
                        }
                        // Second request - receive the token
                        else if request.contains("GET /token?access_token=") {
                            if let Some(token) = request
                                .split("access_token=")
                                .nth(1)
                                .and_then(|s| s.split(' ').next())
                                .and_then(|s| s.split('&').next())
                                .and_then(|s| s.split("HTTP").next())
                                .map(|s| urlencoding::decode(s).unwrap_or_else(|_| s.into()))
                            {
                                let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n\
                                    <html><body><h1>Successfully authenticated!</h1>\
                                    <p>You can close this window and return to the CLI.</p></body></html>";
                                let _ = stream.write_all(response.as_bytes());
                                let _ = tx.send(token.to_string());
                            }
                        }
                    }
                }
            }
        });

        // Open browser after server is ready
        #[cfg(target_os = "macos")]
        Command::new("open").arg(&auth_url).spawn()?;
        #[cfg(target_os = "linux")]
        Command::new("xdg-open").arg(&auth_url).spawn()?;
        #[cfg(target_os = "windows")]
        Command::new("cmd")
            .args(["/C", "start", &auth_url])
            .spawn()?;

        println!("Opening browser for authentication...");
        println!("If the browser doesn't open automatically, please visit:");
        println!("{}", auth_url);

        // Wait for token from callback handler
        if let Ok(token) = rx.recv_timeout(std::time::Duration::from_secs(120)) {
            self.auth_token = Some(token);

            // Get available organizations
            let orgs = self.list_organizations()?;
            match orgs.len() {
                0 => anyhow::bail!("No organizations found for your account"),
                1 => return Ok(orgs[0].clone()),
                _ => {
                    println!("\nMultiple organizations found. Please select one:");
                    for (i, org) in orgs.iter().enumerate() {
                        println!("{}. {} ({})", i + 1, org.name, org.slug);
                    }

                    print!("Enter number (1-{}): ", orgs.len());
                    io::stdout().flush()?;
                    let mut input = String::new();
                    io::stdin().read_line(&mut input)?;
                    let selection = input
                        .trim()
                        .parse::<usize>()
                        .context("Invalid selection")
                        .and_then(|n| {
                            if n > 0 && n <= orgs.len() {
                                Ok(n - 1)
                            } else {
                                Err(anyhow::anyhow!("Selection out of range"))
                            }
                        })?;
                    return Ok(orgs[selection].clone());
                }
            }
        }

        anyhow::bail!("Authentication timed out")
    }

    fn generate_state() -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                                abcdefghijklmnopqrstuvwxyz\
                                0123456789";
        let mut rng = thread_rng();
        (0..32)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    fn get_headers(&self) -> Result<HeaderMap> {
        let auth_token = self
            .auth_token
            .as_ref()
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
        let mut all_projects = Vec::new();
        let cursor: Option<String> = None;

        loop {
            // Build URL with pagination
            let mut url = format!(
                "{}/organizations/{}/projects/?all_projects=1&per_page=100",
                self.base_url, org_slug
            );
            if let Some(cur) = &cursor {
                url.push_str(&format!("&cursor={}", cur));
            }

            let response = self
                .client
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

            let mut page_projects = response
                .json::<Vec<Project>>()
                .context("Failed to parse response")?;

            if page_projects.is_empty() {
                break;
            }

            all_projects.append(&mut page_projects);

            if cursor.is_none() {
                break;
            }
        }

        // Sort projects by name
        all_projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        Ok(all_projects)
    }

    pub fn list_issues(&self, org_slug: &str, project_slug: &str) -> Result<Vec<Issue>> {
        let url = format!(
            "{}/projects/{}/{}/issues/?statsPeriod=14d&query=is:unresolved&sort=date",
            self.base_url, org_slug, project_slug
        );

        let response = self
            .client
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

        response
            .json::<Vec<Issue>>()
            .context("Failed to parse response")
    }

    pub fn get_project_info(
        &self,
        org_slug: &str,
        project_slug: &str,
    ) -> Result<Vec<(String, String)>> {
        let url = format!(
            "{}/projects/{}/{}/?statsPeriod=24h",
            self.base_url, org_slug, project_slug
        );

        let response = self
            .client
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

        let project: Project = response.json().context("Failed to parse response")?;

        // Collect project information
        let mut info = Vec::new();
        info.push(("Name".to_string(), project.name));
        info.push(("Slug".to_string(), project.slug));
        if let Some(platform) = project.platform {
            info.push(("Platform".to_string(), platform));
        }
        if !project.status.is_empty() {
            info.push(("Status".to_string(), project.status));
        }
        if let Some(first) = project.first_event {
            info.push(("First Event".to_string(), first));
        }
        if let Some(last) = project.last_event {
            info.push(("Last Event".to_string(), last));
        }
        if let Some(teams) = project.teams {
            let team_names = teams
                .iter()
                .map(|t| t.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            info.push(("Teams".to_string(), team_names));
        }

        // Add stats if available
        if let Some(stats) = project.stats {
            let total_24h: i64 = stats.last_24h.iter().map(|(_, count)| count).sum();
            let total_30d: i64 = stats.last_30d.iter().map(|(_, count)| count).sum();
            info.push(("Events (24h)".to_string(), total_24h.to_string()));
            info.push(("Events (30d)".to_string(), total_30d.to_string()));

            // Calculate daily average for last 30 days
            let avg_30d = total_30d as f64 / 30.0;
            info.push(("Daily Average (30d)".to_string(), format!("{:.1}", avg_30d)));
        }

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;
    use serde_json::json;

    #[test]
    fn test_client_creation() {
        let server = Server::new();
        let mut client = SentryClient::new().unwrap();
        client.base_url = server.url();
        assert!(client.auth_token.is_none());
    }

    #[test]
    fn test_login() {
        let mut client = SentryClient::new().unwrap();
        client.login("test-token".to_string()).unwrap();
        assert_eq!(client.auth_token, Some("test-token".to_string()));
    }

    #[test]
    fn test_list_projects() -> Result<()> {
        let mut server = Server::new();
        let mock_response = json!([
            {
                "slug": "test-project",
                "name": "Test Project"
            },
            {
                "slug": "another-project",
                "name": "Another Project"
            }
        ]);

        let mock = server
            .mock("GET", "/organizations/test-org/projects/")
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        let mut client = SentryClient {
            client: Client::new(),
            base_url: server.url(),
            auth_token: None,
        };
        client.login("test-token".to_string())?;

        let projects = client.list_projects("test-org")?;
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].slug, "test-project");
        assert_eq!(projects[0].name, "Test Project");
        assert_eq!(projects[1].slug, "another-project");
        assert_eq!(projects[1].name, "Another Project");

        mock.assert();
        Ok(())
    }

    #[test]
    fn test_list_projects_unauthorized() -> Result<()> {
        let mut server = Server::new();

        let mock = server
            .mock("GET", "/organizations/test-org/projects/")
            .match_header("authorization", "Bearer test-token")
            .with_status(401)
            .with_header("content-type", "application/json")
            .with_body(json!({"error": "Unauthorized"}).to_string())
            .create();

        let mut client = SentryClient {
            client: Client::new(),
            base_url: server.url(),
            auth_token: None,
        };
        client.login("test-token".to_string())?;

        let result = client.list_projects("test-org");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("API request failed: 401"));

        mock.assert();
        Ok(())
    }

    #[test]
    fn test_list_issues() -> Result<()> {
        let mut server = Server::new();
        let mock_response = json!([
            {
                "id": "1",
                "title": "Test Issue",
                "status": "unresolved",
                "level": "error",
                "culprit": "test.js:42",
                "lastSeen": "2024-01-01T00:00:00Z",
                "count": 5,
                "userCount": 3
            }
        ]);

        let mock = server
            .mock("GET", "/projects/test-org/test-project/issues/")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("statsPeriod".into(), "14d".into()),
                mockito::Matcher::UrlEncoded("query".into(), "is:unresolved".into()),
                mockito::Matcher::UrlEncoded("sort".into(), "date".into()),
            ]))
            .match_header("authorization", "Bearer test-token")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        let mut client = SentryClient {
            client: Client::new(),
            base_url: server.url(),
            auth_token: None,
        };
        client.login("test-token".to_string())?;

        let issues = client.list_issues("test-org", "test-project")?;
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].id, "1");
        assert_eq!(issues[0].title, "Test Issue");
        assert_eq!(issues[0].status, "unresolved");
        assert_eq!(issues[0].level, "error");
        assert_eq!(issues[0].count, 5);
        assert_eq!(issues[0].user_count, 3);

        mock.assert();
        Ok(())
    }

    #[test]
    fn test_list_issues_not_found() -> Result<()> {
        let mut server = Server::new();

        let mock = server
            .mock("GET", "/projects/test-org/nonexistent-project/issues/")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("statsPeriod".into(), "14d".into()),
                mockito::Matcher::UrlEncoded("query".into(), "is:unresolved".into()),
                mockito::Matcher::UrlEncoded("sort".into(), "date".into()),
            ]))
            .match_header("authorization", "Bearer test-token")
            .with_status(404)
            .with_header("content-type", "application/json")
            .with_body(json!({"error": "Project not found"}).to_string())
            .create();

        let mut client = SentryClient {
            client: Client::new(),
            base_url: server.url(),
            auth_token: None,
        };
        client.login("test-token".to_string())?;

        let result = client.list_issues("test-org", "nonexistent-project");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("API request failed: 404"));

        mock.assert();
        Ok(())
    }

    #[test]
    fn test_unauthenticated_request() {
        let client = SentryClient::new().unwrap();
        let result = client.list_projects("test-org");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Not authenticated"));
    }
}
