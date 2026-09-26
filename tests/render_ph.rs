use qe_monitor::ph;
use ratatui::style::Color;
use ratatui::{Terminal, backend::TestBackend};

mod common;
use common::fixture;

fn chart(name: &str) -> (String, Vec<String>) {
    let metrics = ph::parse_metrics(&fixture(name));
    let tabs = ph::ui::build_tabs(&metrics);
    let mut terminal = Terminal::new(TestBackend::new(50, 10)).unwrap();
    terminal
        .draw(|frame| tabs[0].1.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();

    let mut text = String::new();
    let mut coloured = Vec::new();
    for y in 0..buffer.area.height {
        let mut run = String::new();
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            text.push_str(cell.symbol());
            if matches!(cell.fg, Color::Rgb(..)) {
                run.push_str(cell.symbol());
            }
        }
        if !run.trim().is_empty() {
            coloured.push(run.trim().to_string());
        }
        text.push('\n');
    }
    (text, coloured)
}

#[test]
fn shows_time_per_representation() {
    let name = "ph/metal_multiq.out";
    let (text, coloured) = chart(name);
    assert!(text.contains("0.2s"), "{name}: {text}");
    assert!(text.contains("17 4\u{2588}"), "{name}: {text}");
    assert_eq!(coloured.len(), 8, "{name}: {coloured:?}");
    assert!(
        coloured.iter().all(|c| c.ends_with('s')),
        "{name}: {coloured:?}"
    );
}

#[test]
fn scales_long_representations_to_minutes() {
    let name = "own/ph_midrun.out";
    let (text, coloured) = chart(name);
    assert!(text.contains("3.7m"), "{name}: {text}");
    assert_eq!(coloured, vec!["3.7m", "4.2m"], "{name}: {coloured:?}");
}

#[test]
fn duration_is_drawn_on_top_of_the_bar() {
    let name = "ph/metal_multiq.out";
    let (text, _) = chart(name);
    assert!(
        text.lines().any(|l| l.contains("\u{2588}1.0s")),
        "{name}: duration should sit directly on the bar: {text}"
    );
    for line in text.lines().filter(|l| l.contains('\u{2588}')) {
        let trimmed = line.trim_end_matches(['\u{2502}', ' ']);
        assert!(
            trimmed.ends_with('s') || trimmed.ends_with('m') || trimmed.ends_with('h'),
            "{name}: every bar row should end in its duration: {line}"
        );
    }
}

#[test]
fn duration_keeps_the_bar_visible_behind_it() {
    let name = "ph/metal_multiq.out";
    let metrics = ph::parse_metrics(&fixture(name));
    let tabs = ph::ui::build_tabs(&metrics);
    let mut terminal = Terminal::new(TestBackend::new(50, 10)).unwrap();
    terminal
        .draw(|frame| tabs[0].1.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();

    let mut on_bar = 0;
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            let cell = &buffer[(x, y)];
            if matches!(cell.fg, Color::Rgb(..)) && cell.bg == Color::Gray {
                on_bar += 1;
            }
        }
    }
    assert!(
        on_bar > 0,
        "{name}: the longest bars should show grey behind the duration"
    );
}

#[test]
fn faster_representations_are_greener() {
    let name = "own/ph_midrun.out";
    let metrics = ph::parse_metrics(&fixture(name));
    let per_iter: Vec<f64> = metrics
        .representation_blocks
        .iter()
        .filter_map(|b| b.time_per_iteration())
        .collect();
    assert!(per_iter[0] < per_iter[1], "{name}: {per_iter:?}");

    let tabs = ph::ui::build_tabs(&metrics);
    let mut terminal = Terminal::new(TestBackend::new(50, 6)).unwrap();
    terminal
        .draw(|frame| tabs[0].1.render(frame, frame.area()))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();

    let mut rows = Vec::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            if let Color::Rgb(r, g, _) = buffer[(x, y)].fg {
                rows.push((y, r, g));
                break;
            }
        }
    }
    let (_, fast_r, fast_g) = rows[0];
    let (_, slow_r, slow_g) = rows[1];
    assert!(
        fast_g > fast_r && slow_r > slow_g,
        "{name}: fastest should be green {fast_r},{fast_g} and slowest red {slow_r},{slow_g}"
    );
}

#[test]
fn only_the_duration_is_coloured() {
    let name = "ph/metal_multiq.out";
    let (_, coloured) = chart(name);
    assert!(
        coloured.iter().all(|c| !c.contains('\u{2588}')),
        "{name}: bars must not be coloured: {coloured:?}"
    );
}
