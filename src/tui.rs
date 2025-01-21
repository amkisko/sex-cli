use anyhow::Result;
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{self, ClearType},
    style::Print,
};
use std::io;

pub struct Tui {
    width: u16,
    height: u16,
}

impl Tui {
    pub fn new() -> Result<Self> {
        let (width, height) = terminal::size()?;
        Ok(Self { width, height })
    }

    pub fn start(&self) -> Result<()> {
        terminal::enable_raw_mode()?;
        execute!(
            io::stdout(),
            terminal::EnterAlternateScreen,
            cursor::Hide
        )?;
        Ok(())
    }

    pub fn stop(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::LeaveAlternateScreen,
            cursor::Show
        )?;
        terminal::disable_raw_mode()?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        execute!(
            io::stdout(),
            terminal::Clear(ClearType::All),
            cursor::MoveTo(0, 0)
        )?;
        Ok(())
    }

    pub fn write_at(&self, x: u16, y: u16, text: &str) -> Result<()> {
        execute!(
            io::stdout(),
            cursor::MoveTo(x, y),
            Print(text)
        )?;
        Ok(())
    }

    pub fn read_key(&self) -> Result<KeyEvent> {
        loop {
            if let Event::Key(event) = event::read()? {
                return Ok(event);
            }
        }
    }

    pub fn draw_box(&self, x: u16, y: u16, width: u16, height: u16) -> Result<()> {
        // Draw top border
        self.write_at(x, y, "┌")?;
        for i in 1..width-1 {
            self.write_at(x + i, y, "─")?;
        }
        self.write_at(x + width - 1, y, "┐")?;

        // Draw sides
        for i in 1..height-1 {
            self.write_at(x, y + i, "│")?;
            self.write_at(x + width - 1, y + i, "│")?;
        }

        // Draw bottom border
        self.write_at(x, y + height - 1, "└")?;
        for i in 1..width-1 {
            self.write_at(x + i, y + height - 1, "─")?;
        }
        self.write_at(x + width - 1, y + height - 1, "┘")?;

        Ok(())
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    #[cfg(test)]
    pub fn new_with_size(width: u16, height: u16) -> Self {
        Self { width, height }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_dimensions() {
        let tui = Tui::new_with_size(80, 24);
        assert_eq!(tui.width(), 80);
        assert_eq!(tui.height(), 24);
    }

    #[test]
    fn test_box_dimensions() -> Result<()> {
        let tui = Tui::new_with_size(80, 24);
        tui.draw_box(0, 0, 10, 5)?;
        Ok(())
    }
} 