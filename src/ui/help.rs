use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use super::centered_rect;

pub fn draw(f: &mut Frame, area: Rect) {
    let popup = centered_rect(72, 22, area);
    f.render_widget(Clear, popup);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Help ")
        .border_style(Style::new().fg(Color::Cyan));
    let inner = block.inner(popup);
    f.render_widget(block, popup);

    let lines = vec![
        Line::from(Span::from("Gimbal motion (in Gimbal panel)").bold()),
        kv("W A S D / arrows", "pan & tilt while held (kitty kbd protocol)"),
        kv("Shift + above", "fine motion (5× slower)"),
        kv("+ / - / PageUp / PageDown", "zoom in / out (held)"),
        Line::from(""),
        Line::from(Span::from("Panels & cursor").bold()),
        kv("Tab / Shift+Tab", "next / previous panel"),
        kv("↑ ↓ / k j", "move cursor in Image and Profiles"),
        kv("← → / h l", "decrement / increment value (Image panel)"),
        kv("Enter", "edit numeric value / load selected profile"),
        kv("T", "toggle auto on focused row (wb_auto / focus_auto)"),
        Line::from(""),
        Line::from(Span::from("Profiles").bold()),
        kv("1 .. 9", "quick-load profile slot"),
        kv("Shift+S", "save current state to a new profile"),
        kv("Shift+D", "delete selected profile"),
        kv("Shift+W", "persist profiles to disk now"),
        Line::from(""),
        Line::from(Span::from("Misc").bold()),
        kv("R / Shift+R", "reset all controls to defaults"),
        kv("Esc", "close this help / cancel edit"),
        kv("Q / Ctrl+C", "quit"),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn kv<'a>(k: &'a str, v: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::from(format!("  {k:<28}")).fg(Color::Yellow),
        Span::from(v),
    ])
}
