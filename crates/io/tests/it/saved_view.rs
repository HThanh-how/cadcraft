//! The model view a file was saved with (its `*Active` VPORT) is kept as the view the drawing opens
//! at. A plan view panned away from the origin is saved the way AutoCAD does it: the view centre
//! (code 12) relative to the view target (code 17), the two cancelling each other out, so the
//! centre alone points at the wrong place (a DWG of survey-style sheets opened on a blank screen).

use cadcraft_doc::geom::Vec2;
use cadcraft_doc::{Common, Drawing, EntityKind, Line, Space};
use cadcraft_io::{read_dxf, write_dxf};

/// A `*ACTIVE` VPORT record: the given corners (x0, x1 on the bottom edge; the tile spans the full
/// height), view and these extra tags.
fn record<'a>(x: (&'a str, &'a str), centre: (&'a str, &'a str), height: &'a str, extra: &[(i32, &'a str)]) -> Vec<(i32, &'a str)> {
    let mut tags =
        vec![(0, "VPORT"), (2, "*Active"), (10, x.0), (20, "0"), (11, x.1), (21, "1"), (12, centre.0), (22, centre.1), (40, height), (41, "2.25")];
    tags.extend_from_slice(extra);
    tags
}

fn dxf_of(records: &[Vec<(i32, &str)>]) -> String {
    let mut tags: Vec<(i32, &str)> = vec![(0, "SECTION"), (2, "TABLES"), (0, "TABLE"), (2, "VPORT")];
    tags.extend(records.iter().flatten());
    tags.extend_from_slice(&[(0, "ENDTAB"), (0, "ENDSEC"), (0, "EOF")]);
    tags.iter().map(|(c, v)| format!("{c}\n{v}\n")).collect()
}

/// A DXF with one `*ACTIVE` VPORT record: the unit square, the given view and these extra tags.
fn dxf(centre: (&str, &str), height: &str, extra: &[(i32, &str)]) -> String {
    dxf_of(&[record(("0", "1"), centre, height, extra)])
}

fn saved(dxf: &str) -> Option<(Vec2, f64)> {
    let d = read_dxf(dxf.as_bytes()).expect("read");
    assert!(d.vports.is_empty(), "one *Active viewport is no tiling");
    d.saved_view.map(|t| (t.center, t.height))
}

fn near(a: Vec2, b: Vec2) -> bool {
    (a.x - b.x).abs() < 1e-6 && (a.y - b.y).abs() < 1e-6
}

/// The values of a real AutoCAD DWG of survey-style sheets (read through the DWG bridge): a plan
/// view looking down Z, target far from the origin, centre cancelling it.
#[test]
fn a_plan_view_centre_is_relative_to_the_view_target() {
    let extra = [(16, "0"), (26, "0"), (36, "789.7489"), (17, "666066.5590965878"), (27, "494362.6063628488"), (37, "0"), (51, "0")];
    let (centre, height) = saved(&dxf(("-664932.6046688949", "-493917.19263267587"), "337.6512304264932", &extra)).expect("a saved view");
    assert!(near(centre, Vec2::new(1133.9544276929, 445.41373017293)), "{centre:?}");
    assert_eq!(height, 337.6512304264932);
}

#[test]
fn a_view_without_a_target_keeps_its_centre() {
    // What CADCraft itself writes: direction (0, 0, 1), target at the origin.
    let extra = [(16, "0"), (26, "0"), (36, "1"), (17, "0"), (27, "0"), (37, "0")];
    assert_eq!(saved(&dxf(("12.5", "-7"), "4", &extra)), Some((Vec2::new(12.5, -7.0), 4.0)));
    // Another program that leaves the direction and target out.
    assert_eq!(saved(&dxf(("12.5", "-7"), "4", &[])), Some((Vec2::new(12.5, -7.0), 4.0)));
}

/// A view that is not a plan view without twist or perspective is not what the drawing opens at
/// (CADCraft cannot show it, so it opens at the extents), and a tiled configuration's tile keeps
/// the centre as stored.
#[test]
fn tilted_twisted_and_perspective_views_are_not_opened_at() {
    let target = [(17, "1000"), (27, "2000"), (37, "0")];
    let with = |own: &[(i32, &'static str)]| -> Vec<(i32, &'static str)> { own.iter().copied().chain(target).collect() };
    for (name, extra) in [
        ("tilted", with(&[(16, "1"), (26, "0"), (36, "1")])),
        ("from below", with(&[(16, "0"), (26, "0"), (36, "-1")])),
        ("sideways", with(&[(16, "0"), (26, "1"), (36, "0")])),
        ("twisted", with(&[(16, "0"), (26, "0"), (36, "1"), (51, "30")])),
        ("perspective", with(&[(16, "0"), (26, "0"), (36, "1"), (71, "1")])),
    ] {
        assert_eq!(saved(&dxf(("5", "6"), "4", &extra)), None, "{name}");
    }
    // A full turn is no twist, and the other view mode bits are not perspective.
    let turned = with(&[(16, "0"), (26, "0"), (36, "1"), (51, "360"), (71, "16")]);
    assert_eq!(saved(&dxf(("5", "6"), "4", &turned)).map(|v| v.0), Some(Vec2::new(1005.0, 2006.0)));
    // Two tiles, the first one tilted: the configuration is read, its tile centre as stored.
    let tilted = with(&[(16, "1"), (26, "0"), (36, "1")]);
    let d = read_dxf(dxf_of(&[record(("0", "0.5"), ("5", "6"), "4", &tilted), record(("0.5", "1"), ("7", "8"), "4", &[])]).as_bytes()).expect("read");
    assert_eq!(d.saved_view, None);
    let tiles = &d.vports.first().expect("a configuration").tiles;
    assert_eq!(tiles.iter().map(|t| t.center).collect::<Vec<_>>(), [Vec2::new(5.0, 6.0), Vec2::new(7.0, 8.0)]);
}

#[test]
fn a_hostile_view_is_not_kept() {
    for (centre, height) in [(("0", "0"), "0"), (("0", "0"), "-3"), (("0", "0"), "nan")] {
        assert_eq!(saved(&dxf(centre, height, &[])), None, "{centre:?} {height}");
    }
    // Numbers that are not numbers, a huge target: whatever the reader makes of them, the saved
    // view is finite.
    let extra = [(16, "0"), (26, "0"), (36, "1"), (17, "nan"), (27, "1e308"), (37, "0")];
    for centre in [("nan", "0"), ("0", "nan"), ("1e308", "1e308"), ("-1e308", "1")] {
        for extra in [&[][..], &extra[..]] {
            let view = saved(&dxf(centre, "4", extra));
            assert!(view.is_none_or(|(c, h)| c.is_finite() && h == 4.0), "{centre:?}: {view:?}");
        }
    }
    // No VPORT table at all.
    let none: String = [(0, "SECTION"), (2, "TABLES"), (0, "ENDSEC"), (0, "EOF")].iter().map(|(c, v)| format!("{c}\n{v}\n")).collect();
    assert_eq!(saved(&none), None);
}

/// What CADCraft writes (a single viewport on the extents) opens at the extents, as before.
#[test]
fn a_cadcraft_file_reopens_on_its_extents() {
    let mut d = Drawing::new_imperial();
    let line = Line { a: cadcraft_doc::geom::Vec3::new(10.0, 20.0, 0.0), b: cadcraft_doc::geom::Vec3::new(30.0, 60.0, 0.0) };
    d.add(&Space::Model, Common::default(), EntityKind::Line(line)).expect("add");
    let r = read_dxf(write_dxf(&d).as_bytes()).expect("reopen");
    let v = r.saved_view.expect("a saved view");
    assert!(near(v.center, Vec2::new(20.0, 40.0)), "{:?}", v.center);
    assert!((v.height - 40.0 * 1.1).abs() < 1e-9, "{}", v.height);
}
