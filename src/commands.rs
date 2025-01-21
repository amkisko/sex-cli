use anyhow::Result;
use clap::{Parser, Subcommand};
use crossterm::{
    cursor::{self, Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, Clear, ClearType},
    style::{Color, Print, SetForegroundColor},
};
use std::io::{self, Write};
use crate::config::{Config, Organization};
use crate::issue_viewer::{Issue as ViewerIssue, IssueViewer};
use crate::sentry::SentryClient;
use crate::dashboard::Dashboard;

#[derive(Parser, Debug)]
#[command(author, version, about)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
enum Commands {
    /// Manage organizations
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },
    /// Manage issues
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Login to Sentry
    Login {
        /// Organization name
        org: String,
        /// Authentication token
        token: String,
    },
    /// Monitor issues in real-time
    Monitor {
        /// Organization name
        org: String,
        /// Project slug (defaults to 'default')
        #[arg(default_value = "default")]
        project: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
enum OrgCommands {
    /// List organizations
    List,
    /// Add an organization
    Add {
        /// Organization name
        name: String,
        /// Organization slug
        slug: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
enum IssueCommands {
    /// List issues
    List,
    /// View issue details
    View {
        /// Issue ID
        id: String,
    },
}

impl Cli {
    pub fn run() -> Result<()> {
        let cli = Self::parse();
        let mut config = Config::load()?;
        let mut client = SentryClient::new()?;

        match cli.command {
            Commands::Login { org, token } => {
                let org_entry = config.get_organization_mut(&org)
                    .ok_or_else(|| anyhow::anyhow!("Organization '{}' not found. Add it first with 'org add'.", org))?;

                org_entry.set_auth_token(token)?;
                config.save()?;
                println!("Successfully logged in to Sentry for organization: {}", org);
            }
            Commands::Monitor { org, project } => {
                if !org.is_empty() {
                    let org_entry = config.get_organization(&org)
                        .ok_or_else(|| anyhow::anyhow!("Organization '{}' not found. Add it first with 'org add'.", org))?;

                    let token = org_entry.get_auth_token()?
                        .ok_or_else(|| anyhow::anyhow!("Not logged in for organization '{}'. Use 'login' first.", org))?;

                    client.login(token)?;
                    start_monitor(&client, org_entry.slug.clone(), project)?;
                } else {
                    let mut matches = Vec::new();
                    let mut to_cache = Vec::new();

                    // First pass: collect projects to cache
                    for org in config.organizations.values() {
                        if let Some(token) = org.get_auth_token()? {
                            client.login(token.clone())?;

                            if org.has_project(&project) {
                                matches.push((org.clone(), token));
                            } else if let Ok(projects) = client.list_projects(&org.slug) {
                                if let Some(found_project) = projects.iter().find(|p| p.slug == project) {
                                    to_cache.push((org.name.clone(), project.clone(), found_project.name.clone()));
                                    matches.push((org.clone(), token));
                                }
                            }
                        }
                    }

                    // Second pass: cache projects
                    for (org_name, project_slug, project_name) in to_cache {
                        config.cache_project(&org_name, project_slug, project_name)?;
                    }

                    match matches.len() {
                        0 => {
                            println!("Project '{}' not found in any organization", project);
                            return Ok(());
                        }
                        1 => {
                            let (org, token) = &matches[0];
                            if let Some(Ok(project_name)) = org.get_project(&project) {
                                println!("Found project: {} ({})", project_name, project);
                            }
                            client.login(token.clone())?;
                            start_monitor(&client, org.slug.clone(), project)?;
                        }
                        _ => {
                            let org = select_organization(&matches)?;
                            if let Some(Ok(project_name)) = org.0.get_project(&project) {
                                println!("Selected project: {} ({})", project_name, project);
                            }
                            client.login(org.1.clone())?;
                            start_monitor(&client, org.0.slug.clone(), project)?;
                        }
                    }
                }
            }
            Commands::Org { command } => match command {
                OrgCommands::List => {
                    if config.organizations.is_empty() {
                        println!("No organizations configured");
                    } else {
                        println!("Organizations:");
                        for org in config.organizations.values() {
                            let auth_status = if org.get_auth_token()?.is_some() {
                                "authenticated"
                            } else {
                                "not authenticated"
                            };
                            println!("  {} ({}) - {}", org.name, org.slug, auth_status);

                            // List cached projects
                            for (slug, _) in &org.projects {
                                if let Some(Ok(name)) = org.get_project(slug) {
                                    println!("    - {} ({})", name, slug);
                                }
                            }
                        }
                    }
                }
                OrgCommands::Add { name, slug } => {
                    config.add_organization(name.clone(), slug.clone());
                    config.save()?;
                    println!("Added organization: {} ({})", name, slug);
                }
            },
            Commands::Issue { command } => match command {
                IssueCommands::List => {
                    if config.organizations.is_empty() {
                        println!("No organizations configured. Add one first with 'org add'.");
                        return Ok(());
                    }

                    for org in config.organizations.values() {
                        if let Some(token) = org.get_auth_token()? {
                            client.login(token)?;
                            println!("\nFetching issues for organization: {}", org.name);
                            let issues = client.list_issues(&org.slug, "default")?;

                            if issues.is_empty() {
                                println!("  No issues found");
                            } else {
                                for issue in issues {
                                    println!("  {}: {} ({})", issue.id, issue.title, issue.status);
                                }
                            }
                        }
                    }
                }
                IssueCommands::View { id } => {
                    let mut found = false;
                    for org in config.organizations.values() {
                        if let Some(token) = org.get_auth_token()? {
                            client.login(token)?;
                            if let Ok(issues) = client.list_issues(&org.slug, "default") {
                                if let Some(issue) = issues.into_iter().find(|i| i.id == id) {
                                    found = true;
                                    let viewer_issue = ViewerIssue {
                                        id: issue.id,
                                        title: issue.title,
                                        status: issue.status,
                                        level: issue.level,
                                        culprit: issue.culprit,
                                        last_seen: issue.last_seen,
                                        events: issue.count,
                                        users: issue.user_count,
                                    };

                                    let mut viewer = IssueViewer::new(viewer_issue)?;
                                    viewer.show()?;
                                    break;
                                }
                            }
                        }
                    }
                    if !found {
                        println!("Issue not found in any organization");
                    }
                }
            },
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn parse_from(args: &[&str]) -> Self {
        Self::try_parse_from(args).unwrap()
    }
}

fn start_monitor(client: &SentryClient, org_slug: String, project_slug: String) -> Result<()> {
    println!("Starting monitor for organization: {} project: {}", org_slug, project_slug);
    let mut dashboard = Dashboard::new(
        client.clone(),
        org_slug,
        project_slug,
    );
    dashboard.run()
}

fn select_organization(matches: &[(Organization, String)]) -> Result<(&Organization, String)> {
    println!("\nMultiple organizations have this project. Please select one:");

    terminal::enable_raw_mode()?;
    execute!(io::stdout(), Hide)?;

    let mut selected = 0;
    let mut result = None;

    loop {
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            cursor::MoveTo(0, 0),
            Print("Use arrow keys to select an organization and press Enter:\n\n")
        )?;

        for (i, (org, _)) in matches.iter().enumerate() {
            let prefix = if i == selected { "> " } else { "  " };
            let color = if i == selected { Color::Green } else { Color::Reset };

            execute!(
                io::stdout(),
                SetForegroundColor(color),
                Print(format!("{}{} ({})\n", prefix, org.name, org.slug)),
                SetForegroundColor(Color::Reset)
            )?;
        }

        io::stdout().flush()?;

        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Up if selected > 0 => selected -= 1,
                KeyCode::Down if selected < matches.len() - 1 => selected += 1,
                KeyCode::Enter => {
                    result = Some((&matches[selected].0, matches[selected].1.clone()));
                    break;
                }
                KeyCode::Esc => {
                    println!("Operation cancelled");
                    break;
                }
                _ => {}
            }
        }
    }

    terminal::disable_raw_mode()?;
    execute!(io::stdout(), Show)?;
    println!();

    result.ok_or_else(|| anyhow::anyhow!("No organization selected"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_list_command() {
        let cli = Cli::parse_from(&["sex-cli", "org", "list"]);
        assert!(matches!(cli.command, Commands::Org { command: OrgCommands::List }));
    }

    #[test]
    fn test_org_add_command() {
        let cli = Cli::parse_from(&["sex-cli", "org", "add", "test", "test-slug"]);
        assert!(matches!(
            cli.command,
            Commands::Org {
                command: OrgCommands::Add {
                    name,
                    slug,
                }
            } if name == "test" && slug == "test-slug"
        ));
    }

    #[test]
    fn test_issue_list_command() {
        let cli = Cli::parse_from(&["sex-cli", "issue", "list"]);
        assert!(matches!(cli.command, Commands::Issue { command: IssueCommands::List }));
    }

    #[test]
    fn test_issue_view_command() {
        let cli = Cli::parse_from(&["sex-cli", "issue", "view", "test-id"]);
        assert!(matches!(
            cli.command,
            Commands::Issue {
                command: IssueCommands::View {
                    id,
                }
            } if id == "test-id"
        ));
    }

    #[test]
    fn test_login_command() {
        let cli = Cli::parse_from(&["sex-cli", "login", "test-org", "test-token"]);
        assert!(matches!(
            cli.command,
            Commands::Login { org, token }
            if org == "test-org" && token == "test-token"
        ));
    }

    #[test]
    fn test_monitor_command() {
        let cli = Cli::parse_from(&["sex-cli", "monitor", "test-org"]);
        assert!(matches!(
            cli.command,
            Commands::Monitor { org, project }
            if org == "test-org" && project == "default"
        ));

        let cli = Cli::parse_from(&["sex-cli", "monitor", "test-org", "my-project"]);
        assert!(matches!(
            cli.command,
            Commands::Monitor { org, project }
            if org == "test-org" && project == "my-project"
        ));
    }
} 