use qe_monitor::pw;
use ratatui::{Terminal, backend::TestBackend};

mod common;
use common::fixture;

fn summary(name: &str) -> String {
    let metrics = pw::parse_metrics(&fixture(name));
    let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
    terminal
        .draw(|frame| pw::ui::render_scf_summary(frame, frame.area(), &metrics))
        .unwrap();
    let buffer = terminal.backend().buffer().clone();

    let mut text = String::new();
    for y in 0..buffer.area.height {
        for x in 0..buffer.area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn counts_both_spin_channels_of_an_nscf() {
    // the header says 60 k-points, but that is one spin channel: the band loop runs 120
    let name = "pw/nscf_lsda.out";
    let text = summary(name);
    assert!(text.contains("kpts:           120/120"), "{name}: {text}");
    assert!(!text.contains("pools:"), "{name}: {text}");
}

#[test]
fn counts_kpoints_across_pools() {
    let name = "own/nscf_pools.out";
    let text = summary(name);
    assert!(text.contains("pools:          12"), "{name}: {text}");
}
