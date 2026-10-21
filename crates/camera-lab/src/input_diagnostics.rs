//! Opt-in, bounded native UI diagnostics. Not enabled in release distributions.
use eframe::egui;
use std::{io::Write, sync::{Mutex, OnceLock}};
struct Trace { file: Option<std::fs::File>, lines: usize }
fn line(text: String) {
    static TRACE: OnceLock<Mutex<Trace>> = OnceLock::new();
    let state = TRACE.get_or_init(|| Mutex::new(Trace {
        file: std::env::var_os("KOHDALAB_INPUT_TRACE").and_then(|p|
            std::fs::OpenOptions::new().write(true).create_new(true).open(p).ok()),
        lines: 0,
    }));
    if let Ok(mut state) = state.lock() {
        if state.lines >= 256 { return; }
        let bounded: String = text.chars().take(4096).collect();
        if let Some(file) = &mut state.file { let _ = writeln!(file, "{bounded}"); }
        state.lines += 1;
    }
}
fn summary(events: &[egui::Event]) -> Vec<String> {
    events.iter().take(16).filter_map(|event| match event {
        egui::Event::PointerMoved(p) => Some(format!("PointerMoved({p:?})")),
        egui::Event::MouseMoved(d) => Some(format!("MouseMoved({d:?})")),
        egui::Event::PointerButton { pos, button, pressed, .. } =>
            Some(format!("PointerButton({pos:?}, {button:?}, pressed={pressed})")),
        egui::Event::PointerGone => Some("PointerGone".into()),
        egui::Event::WindowFocused(f) => Some(format!("WindowFocused({f})")),
        egui::Event::Key { key, pressed, .. } if matches!(key,
            egui::Key::F1 | egui::Key::F2 | egui::Key::F3 | egui::Key::F4 |
            egui::Key::F5 | egui::Key::Tab | egui::Key::Enter | egui::Key::Escape) =>
            Some(format!("Navigation({key:?}, pressed={pressed})")),
        _ => None,
    }).collect()
}
pub fn events(ctx: &egui::Context) {
    let ppp = ctx.pixels_per_point();
    ctx.input(|i| {
        let events = summary(&i.events);
        if !events.is_empty() {
            line(format!("events={:?} pointer={:?} down={} ppp={} native_ppp={:?} inner={:?} outer={:?} screen={:?}",
                events, i.pointer.hover_pos(), i.pointer.any_down(), ppp,
                i.viewport().native_pixels_per_point, i.viewport().inner_rect,
                i.viewport().outer_rect, i.screen_rect));
        }
    });
}
pub fn button(ctx: &egui::Context, name: &str, response: &egui::Response) {
    if ctx.input(|i| i.events.iter().any(|e| matches!(e, egui::Event::PointerButton { .. }))) {
        line(format!("button={name} rect={:?} hover={} contains={} down={} clicked={} layer={:?}",
            response.rect, response.hovered(), response.contains_pointer(),
            response.is_pointer_button_down_on(), response.clicked(), response.layer_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_is_redacted_and_event_count_is_bounded() {
        assert!(summary(&[egui::Event::Paste("private".repeat(100000)),
            egui::Event::Text("private".into())]).is_empty());
        assert_eq!(summary(&vec![egui::Event::PointerGone; 1000]).len(),16);
    }
}
