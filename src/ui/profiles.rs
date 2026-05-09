use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};
use ratatui::Frame;

use super::panel_block;
use crate::app::App;

pub fn draw(f: &mut Frame, area: Rect, app: &App, focused: bool) {
    let block = panel_block("Profiles", focused);
    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = app
        .profiles
        .profiles
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let slot = if i < 9 { (b'1' + i as u8) as char } else { ' ' };
            let active_marker = if app.profiles.active.as_deref() == Some(p.name.as_str()) {
                "*"
            } else {
                " "
            };
            ListItem::new(Line::from(vec![
                Span::from(format!(" {slot} ")).dim(),
                Span::from(p.name.clone()),
                Span::from(format!("  {active_marker}")).dim(),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if focused && !app.profiles.profiles.is_empty() {
        state.select(Some(
            app.profile_cursor
                .min(app.profiles.profiles.len().saturating_sub(1)),
        ));
    }

    if items.is_empty() {
        let hint = vec![
            Line::from(""),
            Line::from(Span::from("No profiles yet").dim()),
            Line::from(""),
            Line::from(Span::from("Press ⇧S to save current state").dim()),
        ];
        f.render_widget(ratatui::widgets::Paragraph::new(hint), inner);
        return;
    }

    let list = List::new(items)
        .highlight_style(
            Style::new()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("▶");

    f.render_stateful_widget(list, inner, &mut state);
}
