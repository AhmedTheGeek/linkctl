use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{App, EditMode, StatusLevel};
use crate::input::Panel;

pub mod gimbal;
pub mod help;
pub mod image;
pub mod profiles;

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let footer_h = footer_height(&app.status.text, area.width);
    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(footer_h),
    ])
    .split(area);

    draw_header(f, chunks[0], app);
    draw_main(f, chunks[1], app);
    draw_footer(f, chunks[2], app);

    if let EditMode::Help = app.edit {
        help::draw(f, area);
    }
    if let EditMode::NamingProfile(buf) = &app.edit {
        draw_input_popup(f, area, "Save profile as", buf);
    }
    if let EditMode::EditingValue(_, buf) = &app.edit {
        draw_input_popup(f, area, "Edit value", buf);
    }
    if let EditMode::Confirm(question, _) = &app.edit {
        draw_confirm_popup(f, area, question);
    }
}

fn draw_header(f: &mut Frame, area: Rect, app: &App) {
    let pan_deg = arcsec_to_deg(app.value(crate::camera::CtrlId::Pan).unwrap_or(0));
    let tilt_deg = arcsec_to_deg(app.value(crate::camera::CtrlId::Tilt).unwrap_or(0));
    let zoom = app.value(crate::camera::CtrlId::Zoom).unwrap_or(100) as f64 / 100.0;

    let conn = if app.connected {
        Span::from("● connected").style(Style::new().fg(Color::Green))
    } else {
        Span::from("● disconnected").style(Style::new().fg(Color::Red))
    };

    let kitty = if app.kitty_supported {
        Span::from("kbd:kitty").dim()
    } else {
        Span::from("kbd:legacy").style(Style::new().fg(Color::Yellow))
    };

    let line = Line::from(vec![
        Span::from(" Insta360 Link ").bold(),
        Span::from(app.device_path.clone()).dim(),
        Span::from("   "),
        Span::from(format!("pan {pan_deg:+6.1}°")),
        Span::from("  "),
        Span::from(format!("tilt {tilt_deg:+6.1}°")),
        Span::from("  "),
        Span::from(format!("zoom {zoom:.2}x")),
        Span::from("   "),
        conn,
        Span::from("  "),
        kitty,
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn draw_main(f: &mut Frame, area: Rect, app: &App) {
    let cols = Layout::horizontal([
        Constraint::Percentage(40),
        Constraint::Percentage(35),
        Constraint::Percentage(25),
    ])
    .split(area);
    gimbal::draw(f, cols[0], app, app.focus == Panel::Gimbal);
    image::draw(f, cols[1], app, app.focus == Panel::Image);
    profiles::draw(f, cols[2], app, app.focus == Panel::Profiles);
}

fn draw_footer(f: &mut Frame, area: Rect, app: &App) {
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);

    let hints = Line::from(vec![
        Span::from("WASD/← → ↑ ↓ pan/tilt").dim(),
        Span::from("  +/- zoom").dim(),
        Span::from("  Shift fine").dim(),
        Span::from("  Tab panel").dim(),
        Span::from("  1-9 load").dim(),
        Span::from("  ⇧S save  ⇧D del").dim(),
        Span::from("  T auto").dim(),
        Span::from("  ?:help  Q:quit").dim(),
    ]);
    f.render_widget(Paragraph::new(hints), rows[0]);

    let style = match app.status.level {
        StatusLevel::Info => Style::new().fg(Color::Cyan),
        StatusLevel::Warn => Style::new().fg(Color::Yellow),
        StatusLevel::Error => Style::new().fg(Color::Red),
    };
    let status = Line::from(vec![
        Span::from(" status: ").dim(),
        Span::styled(app.status.text.clone(), style),
    ]);
    f.render_widget(Paragraph::new(status).wrap(Wrap { trim: false }), rows[1]);
}

fn footer_height(status_text: &str, term_width: u16) -> u16 {
    if status_text.is_empty() || term_width == 0 {
        return 2;
    }
    let prefix = " status: ".len() as u16;
    let usable = term_width.saturating_sub(prefix).max(1) as usize;
    let lines = (status_text.chars().count() + usable - 1) / usable;
    let status_lines = (lines as u16).clamp(1, 5);
    1 + status_lines
}

pub fn arcsec_to_deg(v: i64) -> f64 {
    v as f64 / 3600.0
}

pub fn panel_block<'a>(title: &'a str, focused: bool) -> Block<'a> {
    let mut block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "));
    if focused {
        block = block.border_style(Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    }
    block
}

fn draw_input_popup(f: &mut Frame, area: Rect, title: &str, buf: &str) {
    let popup = centered_rect(50, 5, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "));
    let text = Line::from(vec![
        Span::from("> ").dim(),
        Span::from(buf.to_string()),
        Span::from("▏"),
    ]);
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    f.render_widget(Paragraph::new(text), inner);
}

fn draw_confirm_popup(f: &mut Frame, area: Rect, question: &str) {
    let popup = centered_rect(50, 5, area);
    f.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm ")
        .border_style(Style::new().fg(Color::Yellow));
    let inner = block.inner(popup);
    f.render_widget(block, popup);
    let lines = vec![
        Line::from(question.to_string()),
        Line::from(""),
        Line::from(vec![
            Span::from("[y]es").bold(),
            Span::from("   "),
            Span::from("[n]o").bold(),
            Span::from("   "),
            Span::from("Esc cancel").dim(),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), inner);
}

fn centered_rect(width: u16, height: u16, r: Rect) -> Rect {
    let x = r.x + (r.width.saturating_sub(width)) / 2;
    let y = r.y + (r.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(r.width),
        height: height.min(r.height),
    }
}
