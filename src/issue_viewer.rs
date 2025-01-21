use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use crate::tui::Tui;

#[derive(Debug, PartialEq)]
pub struct Issue {
    pub id: String,
    pub title: String,
    pub status: String,
    pub level: String,
    pub culprit: String,
    pub last_seen: String,
    pub events: u32,
    pub users: u32,
}

pub struct IssueViewer {
    tui: Tui,
    issue: Issue,
    scroll_offset: u16,
}

impl IssueViewer {
    pub fn new(issue: Issue) -> Result<Self> {
        Ok(Self {
            tui: Tui::new()?,
            issue,
            scroll_offset: 0,
        })
    }

    #[cfg(test)]
    pub fn new_with_tui(issue: Issue, tui: Tui) -> Self {
        Self {
            tui,
            issue,
            scroll_offset: 0,
        }
    }

    pub fn show(&mut self) -> Result<()> {
        self.tui.start()?;

        loop {
            self.render()?;
            
            match self.tui.read_key()? {
                KeyEvent {
                    code: KeyCode::Char('q'),
                    ..
                } => break,
                KeyEvent {
                    code: KeyCode::Char('j'),
                    ..
                } => self.scroll_down(),
                KeyEvent {
                    code: KeyCode::Char('k'),
                    ..
                } => self.scroll_up(),
                _ => {}
            }
        }

        self.tui.stop()?;
        Ok(())
    }

    fn render(&self) -> Result<()> {
        self.tui.clear()?;

        // Draw main box
        self.tui.draw_box(0, 0, self.tui.width(), self.tui.height())?;

        // Draw title
        self.tui.write_at(2, 1, "Issue Details")?;
        self.tui.write_at(self.tui.width() - 20, 1, "Press 'q' to quit")?;

        // Draw horizontal separator
        for i in 1..self.tui.width()-1 {
            self.tui.write_at(i, 2, "─")?;
        }

        // Draw issue details
        self.tui.write_at(2, 3, &format!("ID: {}", self.issue.id))?;
        self.tui.write_at(2, 4, &format!("Title: {}", self.issue.title))?;
        self.tui.write_at(2, 5, &format!("Status: {}", self.issue.status))?;
        self.tui.write_at(2, 6, &format!("Level: {}", self.issue.level))?;
        self.tui.write_at(2, 7, &format!("Culprit: {}", self.issue.culprit))?;
        self.tui.write_at(2, 8, &format!("Last Seen: {}", self.issue.last_seen))?;
        self.tui.write_at(2, 9, &format!("Events: {}", self.issue.events))?;
        self.tui.write_at(2, 10, &format!("Users Affected: {}", self.issue.users))?;

        // Draw footer
        self.tui.write_at(2, self.tui.height() - 1, "j/k: scroll down/up")?;

        Ok(())
    }

    fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    fn scroll_down(&mut self) {
        // TODO: Add max scroll limit based on content
        self.scroll_offset += 1;
    }

    #[cfg(test)]
    pub fn scroll_offset(&self) -> u16 {
        self.scroll_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_issue() -> Issue {
        Issue {
            id: "test-id".to_string(),
            title: "Test Issue".to_string(),
            status: "unresolved".to_string(),
            level: "error".to_string(),
            culprit: "test.js:42".to_string(),
            last_seen: "2024-01-01".to_string(),
            events: 1,
            users: 1,
        }
    }

    #[test]
    fn test_scroll_up_down() {
        let issue = create_test_issue();
        let tui = Tui::new_with_size(80, 24);
        let mut viewer = IssueViewer::new_with_tui(issue, tui);

        assert_eq!(viewer.scroll_offset(), 0);

        viewer.scroll_down();
        assert_eq!(viewer.scroll_offset(), 1);

        viewer.scroll_down();
        assert_eq!(viewer.scroll_offset(), 2);

        viewer.scroll_up();
        assert_eq!(viewer.scroll_offset(), 1);

        viewer.scroll_up();
        assert_eq!(viewer.scroll_offset(), 0);

        viewer.scroll_up();
        assert_eq!(viewer.scroll_offset(), 0);
    }

    #[test]
    fn test_render() -> Result<()> {
        let issue = create_test_issue();
        let tui = Tui::new_with_size(80, 24);
        let viewer = IssueViewer::new_with_tui(issue, tui);

        viewer.render()?;
        Ok(())
    }
} 