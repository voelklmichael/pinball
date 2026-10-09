//! Runs every scenario in `scenarios/`: first its `expect` checks, then its
//! event trace against the snapshot in `scenarios/snapshots/`.
//!
//! After an intended change, update the snapshots with
//! `INSTA_UPDATE=always cargo test -p scenario` and review them with
//! `git diff scenarios/snapshots` (or `cargo insta review`, if installed).

use std::path::Path;

use engine::layout::Layout;
use scenario::{Recording, Scenario};

/// Draws the ball paths into target/svg/scenarios/<name>.svg, for looking at.
fn write_svg(path: &Path, layout: &Layout, recording: &Recording) {
    const COLORS: [&str; 4] = ["#e41a1c", "#377eb8", "#4daf4a", "#984ea3"];
    let paths: Vec<_> = recording.paths().into_iter().zip(COLORS.iter().cycle()).map(|(p, &c)| (p, c)).collect();
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../target/svg/scenarios");
    std::fs::create_dir_all(dir).unwrap();
    let name = path.file_stem().unwrap().to_string_lossy();
    std::fs::write(format!("{dir}/{name}.svg"), engine::svg::render(layout, &paths)).unwrap();
}

#[test]
fn scenarios() {
    insta::glob!("../../scenarios", "*.json", |path| {
        let (scenario, layout) = Scenario::load(path).unwrap();
        let svg_layout = Scenario::load(path).unwrap().1;
        let recording = scenario.run(layout);
        write_svg(path, &svg_layout, &recording);
        let failures = scenario.check(&recording);
        assert!(
            failures.is_empty(),
            "{} failed:\n  {}\ntrace:\n{}",
            path.display(),
            failures.join("\n  "),
            recording.trace()
        );

        let mut settings = insta::Settings::clone_current();
        settings.set_snapshot_path("../../scenarios/snapshots");
        settings.set_prepend_module_to_snapshot(false);
        settings.set_description(scenario.description.as_str());
        settings.set_omit_expression(true);
        settings.bind(|| insta::assert_snapshot!(recording.trace()));
    });
}
