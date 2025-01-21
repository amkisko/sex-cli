use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, ClearType},
    style::{Color, Print, SetForegroundColor},
};
use std::io::{self, Write};
use std::time::Duration;
use crate::sentry::{SentryClient, Issue};

pub struct Dashboard {
    client: SentryClient,
    org_slug: String,
    project_slug: String,
    issues: Vec<Issue>,
    selected_index: usize,
}

impl Dashboard {
    pub fn new(client: SentryClient, org_slug: String, project_slug: String) -> Self {
        Self {
            client,
            org_slug,
            project_slug,
            issues: Vec::new(),
            selected_index: 0,
        }
    }

    pub fn run(&mut self) -> Result<()> {
        self.setup_terminal()?;

        let mut last_update = std::time::Instant::now();
        let update_interval = Duration::from_secs(5);

        loop {
            if last_update.elapsed() >= update_interval {
                self.update_issues()?;
                last_update = std::time::Instant::now();
            }

            self.render()?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Up => self.move_selection_up(),
                        KeyCode::Down => self.move_selection_down(),
                        _ => {}
                    }
                }
            }
        }

        self.cleanup_terminal()?;
        Ok(())
    }

    fn setup_terminal(&self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide
        )?;
        Ok(())
    }

    fn cleanup_terminal(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::LeaveAlternateScreen,
            cursor::Show
        )?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    fn update_issues(&mut self) -> Result<()> {
        let mut issues = self.client.list_issues(&self.org_slug, &self.project_slug)?;
        issues.sort_by(|a, b| b.count.cmp(&a.count));
        self.issues = issues.into_iter().take(10).collect();
        Ok(())
    }

    fn render(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;

        // Header
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Cyan),
            Print("Sentry Issue Monitor - Press 'q' to quit\n\n"),
            SetForegroundColor(Color::Reset)
        )?;

        // Column headers
        execute!(
            io::stdout(),
            SetForegroundColor(Color::Yellow),
            Print(format!("{:<10} {:<40} {:<12} {:<8} {:<8}\n",
                "ID", "Title", "Status", "Events", "Users")),
            SetForegroundColor(Color::Reset)
        )?;

        // Issues
        for (index, issue) in self.issues.iter().enumerate() {
            let color = if index == self.selected_index {
                Color::Green
            } else {
                Color::Reset
            };

            let id_short = &issue.id[..10.min(issue.id.len())];
            let title_short = if issue.title.len() > 40 {
                format!("{}...", &issue.title[..37])
            } else {
                issue.title.clone()
            };

            execute!(
                io::stdout(),
                SetForegroundColor(color),
                Print(format!("{:<10} {:<40} {:<12} {:<8} {:<8}\n",
                    id_short,
                    title_short,
                    issue.status,
                    issue.count,
                    issue.user_count
                )),
                SetForegroundColor(Color::Reset)
            )?;
        }

        io::stdout().flush()?;
        Ok(())
    }

    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
    }

    fn move_selection_down(&mut self) {
        if !self.issues.is_empty() && self.selected_index < self.issues.len() - 1 {
            self.selected_index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dashboard_creation() {
        let client = SentryClient::new().unwrap();
        let dashboard = Dashboard::new(
            client,
            "test-org".to_string(),
            "test-project".to_string()
        );
        assert_eq!(dashboard.selected_index, 0);
        assert!(dashboard.issues.is_empty());
    }
} 