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
            // The duration text is the gradient colour, never a neutral grey;
            // the bars are grey, so unequal channels single out the text.
            if let Color::Rgb(r, g, b) = cell.fg
                && !(r == g && g == b)
            {
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
            if matches!(cell.fg, Color::Rgb(..)) && matches!(cell.bg, Color::Rgb(..)) {
                on_bar += 1;
            }
        }
    }
    assert!(
        on_bar > 0,
        "{name}: the longest bars should show their shade behind the duration"
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
            if let Color::Rgb(r, g, b) = buffer[(x, y)].fg
                && !(r == g && g == b)
            {
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

fn threshold_row(fixture_name: &str) -> (String, Option<Color>) {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(fixture_name);
    let app = qe_monitor::app::App::new(path, qe_monitor::CalcType::Ph);
    let mut terminal = Terminal::new(TestBackend::new(150, 30)).unwrap();
    terminal
        .draw(|frame| qe_monitor::ui::ui(frame, &app))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();

    for y in 0..buffer.area.height {
        let line: String = (0..buffer.area.width)
            .map(|x| buffer[(x, y)].symbol())
            .collect();
        if let Some(start) = line.find("|ddv_scf|") {
            let colour = (start as u16..buffer.area.width)
                .map(|x| buffer[(x, y)].fg)
                .find(|c| matches!(c, Color::LightGreen | Color::LightRed));
            return (line.trim().to_string(), colour);
        }
    }
    panic!("{fixture_name}: no threshold row rendered");
}

#[test]
fn phonon_panel_reports_the_convergence_threshold() {
    let (row, colour) = threshold_row("ph/base_si.out");
    assert!(row.contains("1.00e-14"), "target missing: {row}");
    assert!(row.contains("4.93e-15"), "current missing: {row}");
    assert_eq!(colour, Some(Color::LightGreen), "converged: {row}");

    let (row, colour) = threshold_row("own/ph_midrun.out");
    assert!(row.contains("1.00e-14"), "target missing: {row}");
    assert!(row.contains("1.12e-13"), "current missing: {row}");
    assert_eq!(colour, Some(Color::LightRed), "not converged: {row}");
}

#[test]
fn phonon_panel_omits_the_current_value_before_any_iteration() {
    let (row, colour) = threshold_row("ph/restart1.out");
    assert!(row.contains("1.00e-18"), "target missing: {row}");
    assert_eq!(colour, None, "nothing to compare yet: {row}");
}
