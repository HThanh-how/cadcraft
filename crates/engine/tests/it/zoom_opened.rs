//! A drawing opens at the view its file was saved with, not always at its extents: a few objects
//! far from the rest make the extents a view where everything else is a speck. A saved view that
//! shows nothing of the drawing (a stale one) falls back to the extents.

use cadcraft_engine::doc::{Common, Drawing, EntityKind, Layer, Line, Space, VportTile};
use cadcraft_engine::geom::{Vec2, Vec3};
use cadcraft_engine::{Session, View};

fn line(d: &mut Drawing, a: (f64, f64), b: (f64, f64), layer: &str) {
    let common = Common { layer: layer.into(), ..Common::default() };
    d.add(&Space::Model, common, EntityKind::Line(Line { a: Vec3::new(a.0, a.1, 0.0), b: Vec3::new(b.0, b.1, 0.0) })).expect("add");
}

fn saved(center: (f64, f64), height: f64) -> Option<VportTile> {
    Some(VportTile { min: Vec2::ZERO, max: Vec2::new(1.0, 1.0), center: Vec2::new(center.0, center.1), height, aspect: 1.5 })
}

/// A drawing of two sheets 800 000 units apart: its extents are thousands of times higher than a
/// view of one of them.
fn sheets(saved_view: Option<VportTile>) -> Drawing {
    let mut d = Drawing::new_metric();
    line(&mut d, (0.0, 0.0), (100.0, 50.0), "0");
    line(&mut d, (800_000.0, 600_000.0), (800_100.0, 600_050.0), "0");
    d.saved_view = saved_view;
    d
}

fn open_in(d: Drawing, px: (f64, f64)) -> Session {
    let mut s = Session::new();
    s.viewport_px = px;
    s.open_drawing(d, "Drawing", None);
    s
}

/// [`sheets`], opened in a 1200 x 800 pixel drawing area.
fn open(saved_view: Option<VportTile>) -> Session {
    open_in(sheets(saved_view), (1200.0, 800.0))
}

fn view(s: &Session) -> View {
    s.state().expect("a drawing").view()
}

#[test]
fn opens_at_the_saved_view_when_it_shows_the_drawing() {
    let mut s = open(saved((50.0, 25.0), 80.0));
    // Opening the drawing already shows it (the typed OPEN, scripts and the control channel never
    // ask for a zoom), and the app's own zoom after the first frame keeps it.
    assert_eq!(view(&s), View { center: Vec2::new(50.0, 25.0), height: 80.0 });
    s.zoom_opened().expect("zoom");
    assert_eq!(view(&s), View { center: Vec2::new(50.0, 25.0), height: 80.0 });
}

/// The extents of a drawing without a saved view, and of one whose saved view shows none of it,
/// are as before.
#[test]
fn opening_without_a_usable_saved_view_fits_the_extents() {
    let fitted = view(&open(None));
    assert!(fitted.height > 600_000.0, "{fitted:?}");
    assert_eq!(view(&open(saved((400_000.0, 300_000.0), 80.0))), fitted);
}

#[test]
fn opens_at_the_extents_without_a_saved_view() {
    let mut s = open(None);
    s.zoom_opened().expect("zoom");
    let mut e = open(None);
    e.zoom_extents().expect("zoom");
    assert_eq!(view(&s), view(&e));
    assert!(view(&s).height > 600_000.0, "the extents take in both sheets");
}

#[test]
fn a_saved_view_that_shows_nothing_falls_back_to_the_extents() {
    // Between the sheets, and far away from both.
    for center in [(400_000.0, 300_000.0), (-5_000_000.0, 7.0)] {
        let mut s = open(saved(center, 80.0));
        s.zoom_opened().expect("zoom");
        let mut e = open(None);
        e.zoom_extents().expect("zoom");
        assert_eq!(view(&s), view(&e), "{center:?}");
    }
}

#[test]
fn hidden_objects_do_not_count_as_shown() {
    let mut d = Drawing::new_metric();
    d.layers.push(Layer { name: "Off".into(), on: false, ..Layer::default() });
    line(&mut d, (0.0, 0.0), (100.0, 50.0), "Off");
    line(&mut d, (5_000.0, 5_000.0), (5_100.0, 5_050.0), "0");
    line(&mut d, (800_000.0, 600_000.0), (800_100.0, 600_050.0), "0");
    d.saved_view = saved((50.0, 25.0), 80.0);
    let mut s = open_in(d, (1200.0, 800.0));
    assert!(view(&s).center.x > 1_000.0, "the extents of what is visible: {:?}", view(&s));
    s.zoom_opened().expect("zoom");
    assert!(view(&s).center.x > 1_000.0, "the extents of what is visible: {:?}", view(&s));
}

/// A drawing opens at its saved view only when its extents are 10 times higher than that view
/// (a few objects far away). Otherwise it is fitted to its extents, as it always was.
#[test]
fn a_saved_view_not_much_smaller_than_the_extents_is_not_used() {
    // Extents 2000 x 1000, which take 1467 units of height in a 1.5 : 1 drawing area.
    let wide = |saved_view: Option<VportTile>| {
        let mut d = Drawing::new_metric();
        line(&mut d, (0.0, 0.0), (2000.0, 1000.0), "0");
        d.saved_view = saved_view;
        open_in(d, (1200.0, 800.0))
    };
    let fitted = view(&wide(None));
    assert_eq!(view(&wide(saved((1000.0, 500.0), 200.0))), fitted, "7 times smaller");
    assert_eq!(view(&wide(saved((1000.0, 500.0), 100.0))), View { center: Vec2::new(1000.0, 500.0), height: 100.0 }, "15 times smaller");
    // The app's zoom after the first frame agrees.
    let mut s = wide(saved((1000.0, 500.0), 200.0));
    s.zoom_opened().expect("zoom");
    let mut e = wide(None);
    e.zoom_extents().expect("zoom");
    assert_eq!(view(&s), view(&e));
}

/// The file saved a view 80 high and 120 wide (aspect 1.5). A drawing area that is wider keeps the
/// height; one that is narrower is shown the whole width the author saw, so the view gets higher.
#[test]
fn a_narrow_drawing_area_still_shows_the_saved_width() {
    for (px, height) in [((1200.0, 800.0), 80.0), ((4000.0, 200.0), 80.0), ((600.0, 800.0), 160.0), ((300.0, 900.0), 360.0)] {
        let mut s = open_in(sheets(saved((50.0, 25.0), 80.0)), px);
        assert!((view(&s).height - height).abs() < 1e-9, "open {px:?}: {:?}", view(&s));
        s.zoom_opened().expect("zoom");
        assert!((view(&s).height - height).abs() < 1e-9, "zoom {px:?}: {:?}", view(&s));
        assert_eq!(view(&s).center, Vec2::new(50.0, 25.0));
    }
    // A file that does not say how wide its window was keeps the height whatever the shape.
    let unknown = Some(VportTile { aspect: 0.0, ..saved((50.0, 25.0), 80.0).expect("a tile") });
    assert_eq!(view(&open_in(sheets(unknown), (300.0, 900.0))).height, 80.0);
}

#[test]
fn hostile_saved_views_never_panic() {
    for (center, height) in
        [((f64::NAN, 0.0), 10.0), ((0.0, f64::INFINITY), 10.0), ((1e308, -1e308), 1e308), ((0.0, 0.0), f64::MAX), ((0.0, 0.0), 1e-300)]
    {
        let mut s = open(saved(center, height));
        s.zoom_opened().expect("zoom");
        let v = view(&s);
        assert!(v.center.is_finite() && v.height.is_finite() && v.height > 0.0, "{center:?} {height}: {v:?}");
    }
}

/// The whole way from a file: a VPORT as AutoCAD saves a panned plan view (the centre relative to
/// the view target), and a few objects 850 000 units from the rest, which make the extents a view
/// where the drawing is a speck (a real DWG of fire-safety sheets opened on a blank screen).
#[test]
fn a_file_with_far_objects_opens_where_it_was_saved() {
    let tags: &[(i32, &str)] = &[
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "VPORT"),
        (0, "VPORT"),
        (2, "*Active"),
        (10, "0"),
        (20, "0"),
        (11, "1"),
        (21, "1"),
        (12, "-664932.6"),
        (22, "-493917.2"),
        (16, "0"),
        (26, "0"),
        (36, "789.7"),
        (17, "666066.5"),
        (27, "494362.6"),
        (37, "0"),
        (40, "337.65"),
        (41, "2.28"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (8, "0"),
        (10, "1000"),
        (20, "400"),
        (30, "0"),
        (11, "1200"),
        (21, "500"),
        (31, "0"),
        (0, "LINE"),
        (8, "0"),
        (10, "856927"),
        (20, "593795"),
        (30, "0"),
        (11, "856966"),
        (21, "593815"),
        (31, "0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    let dxf: String = tags.iter().map(|(c, v)| format!("{c}\n{v}\n")).collect();
    let d = cadcraft_io::read_dxf(dxf.as_bytes()).expect("read");
    let mut s = Session::new();
    s.viewport_px = (1200.0, 800.0);
    s.open_drawing(d, "Far", None);
    let v = view(&s);
    assert!((v.center.x - 1133.9).abs() < 1e-6 && (v.center.y - 445.4).abs() < 1e-6, "{v:?}");
    // 337.65 high and 2.28 wide as saved; this area is 1.5 wide, so the saved width sets the height.
    assert!((v.height - 337.65 * 2.28 / 1.5).abs() < 1e-6, "{v:?}");
}
