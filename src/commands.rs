use crate::config::{Config, Organization};
use crate::dashboard::Dashboard;
use crate::issue_viewer::{Issue as ViewerIssue, IssueViewer};
use crate::sentry::SentryClient;
use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{generate, Shell};
use crossterm::{
    cursor::{self, Hide, Show},
    event::{self, Event, KeyCode},
    execute,
    style::{Color, Print, SetForegroundColor},
    terminal::{self, Clear, ClearType},
};
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Sentry Explorer CLI - A tool for exploring and monitoring Sentry issues"
)]
#[command(
    long_about = "A command-line interface tool for exploring Sentry issues and data, \
    with support for multiple organizations, real-time monitoring, and encrypted token storage."
)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
enum Commands {
    /// Manage Sentry organizations
    #[command(about = "Manage Sentry organizations and their settings")]
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },
    /// Manage Sentry projects
    #[command(about = "View and manage Sentry projects", alias = "p")]
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },
    /// Manage Sentry issues
    #[command(
        about = "View and manage Sentry issues across organizations",
        alias = "i"
    )]
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Login to a Sentry organization
    #[command(about = "Authenticate with a Sentry organization")]
    Login {
        /// Use browser-based OAuth login instead of token
        #[arg(long, help = "Use browser-based OAuth login flow")]
        browser: bool,
        /// Organization name (optional, will be detected automatically if not provided)
        #[arg(help = "Name of the organization to authenticate with")]
        org: Option<String>,
    },
    /// Monitor issues in real-time
    #[command(
        about = "Start a real-time dashboard for monitoring Sentry issues",
        alias = "m"
    )]
    Monitor {
        /// Organization and project in format: [org/]project
        #[arg(
            help = "Project to monitor in format: [org/]project (e.g. 'my-org/my-project' or just 'my-project')"
        )]
        target: String,
    },
    /// Generate shell completions
    #[command(about = "Generate shell completion scripts")]
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
enum OrgCommands {
    /// List configured organizations
    #[command(about = "List all configured organizations and their authentication status")]
    List,
    /// Add a new organization
    #[command(about = "Add a new Sentry organization to the configuration")]
    Add {
        /// Organization name (used for local reference)
        #[arg(help = "Name to identify the organization locally")]
        name: String,
        /// Organization slug (from Sentry URL)
        #[arg(
            help = "Organization slug from Sentry URL (e.g., 'my-org' from sentry.io/organizations/my-org/)"
        )]
        slug: String,
    },
    /// List organization projects
    #[command(about = "List all projects in an organization")]
    Projects {
        /// Organization name
        #[arg(help = "Name of the organization")]
        name: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
enum ProjectCommands {
    /// List all projects across organizations
    #[command(about = "List all projects from all authenticated organizations")]
    List,
    /// Show project information
    #[command(about = "Show detailed project information including stats")]
    Info {
        /// Project identifier in format: [org/]project
        #[arg(
            help = "Project to show in format: [org/]project (e.g. 'my-org/my-project' or just 'my-project')"
        )]
        target: String,
    },
}

#[derive(Subcommand, Debug, PartialEq)]
enum IssueCommands {
    /// List recent issues
    #[command(about = "List recent unresolved issues from all authenticated organizations")]
    List,
    /// View detailed issue information
    #[command(about = "View detailed information about a specific issue in an interactive viewer")]
    View {
        /// Issue ID
        #[arg(help = "Issue ID from Sentry (found in issue URL or list command)")]
        id: String,
    },
}

impl Cli {
    pub fn run() -> Result<()> {
        let cli = Self::parse();
        let mut config = Config::load()?;
        let mut client = SentryClient::new()?;

        match cli.command {
            Commands::Login { browser, org } => {
                if browser {
                    let sentry_org = client.login_with_browser()?;
                    let org_name = org.unwrap_or_else(|| sentry_org.slug.clone());
                    // Add organization if it doesn't exist
                    if !config.organizations.contains_key(&org_name) {
                        config.add_organization(org_name.clone(), sentry_org.slug);
                        println!("Added new organization: {}", org_name);
                    }

                    let org_entry = config.get_organization_mut(&org_name).unwrap();
                    if let Some(token) = client.get_current_token() {
                        org_entry.set_auth_token(token)?;
                        config.save()?;
                        println!(
                            "Successfully logged in to Sentry for organization: {}",
                            org_name
                        );
                    }
                } else {
                    let org = org.ok_or_else(|| {
                        anyhow::anyhow!("Organization name is required for token-based login")
                    })?;
                    let org_entry = config.get_organization_mut(&org).ok_or_else(|| {
                        anyhow::anyhow!(
                            "Organization '{}' not found. Add it first with 'org add'.",
                            org
                        )
                    })?;

                    client.login_with_prompt()?;
                    if let Some(token) = client.get_current_token() {
                        org_entry.set_auth_token(token)?;
                        config.save()?;
                        println!("Successfully logged in to Sentry for organization: {}", org);
                    }
                }
            }
            Commands::Monitor { target } => {
                let (org, project) = if let Some((org_part, project_part)) = target.split_once('/')
                {
                    (org_part.to_string(), project_part.to_string())
                } else {
                    (String::new(), target)
                };

                if !org.is_empty() {
                    let org_entry = config.get_organization(&org).ok_or_else(|| {
                        anyhow::anyhow!(
                            "Organization '{}' not found. Add it first with 'org add'.",
                            org
                        )
                    })?;

                    let token = org_entry.get_auth_token()?.ok_or_else(|| {
                        anyhow::anyhow!(
                            "Not logged in for organization '{}'. Use 'login' first.",
                            org
                        )
                    })?;

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
                                if let Some(found_project) =
                                    projects.iter().find(|p| p.slug == project)
                                {
                                    to_cache.push((
                                        org.name.clone(),
                                        project.clone(),
                                        found_project.name.clone(),
                                    ));
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
                            let matches_owned: Vec<(Organization, String)> = matches
                                .into_iter()
                                .map(|(org, token)| (org.clone(), token.clone()))
                                .collect();
                            let org = select_organization(&matches_owned[..])?;
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
                OrgCommands::Projects { name } => {
                    let org = config
                        .get_organization(&name)
                        .ok_or_else(|| anyhow::anyhow!("Organization '{}' not found", name))?;
                    println!("Projects in organization: {}", name);
                    for project in org.projects.keys() {
                        println!("  - {}", project);
                    }
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
            Commands::Project { command } => match command {
                ProjectCommands::List => {
                    if config.organizations.is_empty() {
                        println!("No organizations configured. Add one first with 'org add'.");
                        return Ok(());
                    }

                    for org in config.organizations.values() {
                        if let Some(token) = org.get_auth_token()? {
                            client.login(token)?;
                            println!("\nProjects in organization: {}", org.name);
                            let projects = client.list_projects(&org.slug)?;

                            if projects.is_empty() {
                                println!("  No projects found");
                            } else {
                                for project in projects {
                                    let platform =
                                        project.platform.unwrap_or_else(|| "-".to_string());
                                    let access = if project.hasAccess.unwrap_or(false) {
                                        "✓"
                                    } else {
                                        "✗"
                                    };
                                    println!(
                                        "  {} {} [{}] {}",
                                        access, project.name, platform, project.slug
                                    );
                                }
                            }
                        }
                    }
                }
                ProjectCommands::Info { target } => {
                    let (org, project) =
                        if let Some((org_part, project_part)) = target.split_once('/') {
                            (org_part.to_string(), project_part.to_string())
                        } else {
                            (String::new(), target)
                        };

                    if !org.is_empty() {
                        let org_entry = config.get_organization(&org).ok_or_else(|| {
                            anyhow::anyhow!(
                                "Organization '{}' not found. Add it first with 'org add'.",
                                org
                            )
                        })?;

                        let token = org_entry.get_auth_token()?.ok_or_else(|| {
                            anyhow::anyhow!(
                                "Not logged in for organization '{}'. Use 'login' first.",
                                org
                            )
                        })?;

                        client.login(token)?;
                        start_project_info(&client, org_entry.slug.clone(), project)?;
                    } else {
                        println!("Project identifier must include organization");
                    }
                }
            },
            Commands::Completion { shell } => {
                let mut cmd = Self::command();
                let bin_name = cmd.get_name().to_string();
                generate(shell, &mut cmd, bin_name, &mut io::stdout());
            }
        }

        Ok(())
    }

    #[cfg(test)]
    pub fn parse_from(args: &[&str]) -> Self {
        Self::try_parse_from(args).unwrap()
    }
}

fn start_monitor(client: &SentryClient, org_slug: String, project_slug: String) -> Result<()> {
    println!(
        "Starting monitor for organization: {} project: {}",
        org_slug, project_slug
    );
    let mut dashboard = Dashboard::new(client.clone(), org_slug, project_slug);
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
            let color = if i == selected {
                Color::Green
            } else {
                Color::Reset
            };

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

fn start_project_info(client: &SentryClient, org_slug: String, project_slug: String) -> Result<()> {
    println!(
        "Starting project info for organization: {} project: {}",
        org_slug, project_slug
    );
    let project_info = client.get_project_info(&org_slug, &project_slug)?;
    println!("Project Info:");
    for (key, value) in project_info {
        println!("  {}: {}", key, value);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_org_list_command() {
        let cli = Cli::parse_from(&["sex-cli", "org", "list"]);
        assert!(matches!(
            cli.command,
            Commands::Org {
                command: OrgCommands::List
            }
        ));
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
        assert!(matches!(
            cli.command,
            Commands::Issue {
                command: IssueCommands::List
            }
        ));
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
        let cli = Cli::parse_from(&["sex-cli", "login", "test-org"]);
        assert!(matches!(
            cli.command,
            Commands::Login { org }
            if org == "test-org"
        ));
    }

    #[test]
    fn test_monitor_command() {
        // Test project-only format
        let cli = Cli::parse_from(&["sex-cli", "monitor", "my-project"]);
        assert!(matches!(
            cli.command,
            Commands::Monitor { target }
            if target == "my-project"
        ));

        // Test org/project format
        let cli = Cli::parse_from(&["sex-cli", "monitor", "test-org/my-project"]);
        assert!(matches!(
            cli.command,
            Commands::Monitor { target }
            if target == "test-org/my-project"
        ));
    }

    #[test]
    fn test_project_list_command() {
        let cli = Cli::parse_from(&["sex-cli", "project", "list"]);
        assert!(matches!(
            cli.command,
            Commands::Project {
                command: ProjectCommands::List
            }
        ));
    }

    #[test]
    fn test_project_info_command() {
        let cli = Cli::parse_from(&["sex-cli", "project", "info", "test-org/my-project"]);
        assert!(matches!(
            cli.command,
            Commands::Project {
                command: ProjectCommands::Info {
                    target,
                }
            } if target == "test-org/my-project"
        ));
    }
}
