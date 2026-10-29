use super::{Bounds2, Result, Session, Space, Vec2, View};

impl Session {
    // ---------------- views ----------------

    /// The limits of the space being edited: the sheet in paper space, else LIMMIN/LIMMAX.
    pub fn zoom_limits(&self) -> Result<Bounds2> {
        let d = self.doc()?;
        if let Space::Paper(name) = self.space()
            && let Some(sheet) = cadcraft_render::sheet(d, &name)
        {
            return Ok(sheet.bounds());
        }
        let lo = d.header.point("LIMMIN").map(|p| p.xy()).unwrap_or(Vec2::ZERO);
        let hi = d.header.point("LIMMAX").map(|p| p.xy()).unwrap_or(Vec2::new(12.0, 9.0));
        Ok(Bounds2::new(lo, hi))
    }

    pub fn zoom_extents(&mut self) -> Result<()> {
        let space = self.space();
        let ext = self.doc()?.extents(&space);
        let (w, h) = self.viewport_px;
        let ext = if ext.is_empty() { self.zoom_limits()? } else { ext };
        let st = self.state_mut()?;
        let aspect = st.zoom_aspect(w / h.max(1.0));
        let height = ext.height().max(ext.width() / aspect.max(1e-6)).max(1e-6) * 1.05;
        st.set_zoom_frame(View { center: ext.center(), height });
        Ok(())
    }

    /// The view a drawing starts at once it is open: the model view its file was saved with (see
    /// [`saved_open_view`]), else the extents. The drawing area may not have its size yet when a
    /// file is opened, so the app asks again at the first frame.
    pub fn zoom_opened(&mut self) -> Result<()> {
        let space = self.space();
        let ext = self.doc()?.extents(&space);
        match saved_open_view(self.doc()?, &space, &ext, self.viewport_px) {
            Some(v) => {
                self.state_mut()?.set_zoom_frame(v);
                Ok(())
            }
            None => self.zoom_extents(),
        }
    }

    /// Zoom by `factor` (> 1 zooms in) keeping `about` fixed on screen.
    pub fn zoom_about(&mut self, factor: f64, about: Vec2) -> Result<()> {
        if !(factor.is_finite() && factor > 0.0) {
            return Ok(());
        }
        let st = self.state_mut()?;
        let v = st.view();
        let h = (v.height / factor).clamp(1e-9, 1e12);
        let k = h / v.height;
        let c = about + (v.center - about) * k;
        st.set_view_quiet(View { center: c, height: h });
        Ok(())
    }
}

/// How many times higher than the view its file was saved with the extents must be for a drawing
/// to open at that view. Most drawings open fitted to their extents, as the saved view of a file
/// that was not zoomed in much is nearly the extents anyway (and one saved by an older version may
/// crop a little of the drawing). Past this, the extents show the drawing a tenth of the size its
/// author saw it, which is a few far-away objects stretching the extents.
pub(crate) const SAVED_VIEW_MIN_ZOOM: f64 = 10.0;

/// The model view `d`'s file was saved with (the `*Active` viewport, as AutoCAD restores it) for a
/// drawing area of `px` pixels whose model space has extents `ext`, when the drawing should open
/// at it: `None` for a drawing without one, in paper space, when the extents are not
/// [`SAVED_VIEW_MIN_ZOOM`] times higher than the view, or when the view shows none of the
/// drawing's visible objects (a stale view); the caller shows the extents. A drawing with a few
/// objects far from the rest has extents that make everything else a speck; its saved view is
/// where the author was working. The view is widened when the drawing area is narrower than the
/// window the file was saved from, so the whole width the author saw is shown.
pub(crate) fn saved_open_view(d: &super::Drawing, space: &Space, ext: &Bounds2, px: (f64, f64)) -> Option<View> {
    let saved = d.saved_view.filter(|v| !space.is_paper() && v.center.is_finite() && v.height.is_finite() && v.height > 0.0)?;
    let aspect = (px.0 / px.1.max(1.0)).max(1e-6);
    let saved_aspect = if saved.aspect.is_finite() && saved.aspect > 0.0 { saved.aspect } else { 0.0 };
    let height = saved.height.max(saved.height * saved_aspect / aspect);
    let fitted = ext.height().max(ext.width() / aspect) * 1.1;
    if !(ext.height().is_finite() && fitted > SAVED_VIEW_MIN_ZOOM * height) {
        return None;
    }
    let half = Vec2::new(height * aspect, height) * 0.5;
    let area = Bounds2::new(saved.center - half, saved.center + half);
    (height.is_finite() && area.min.is_finite() && area.max.is_finite() && d.shows_entities(space, &area))
        .then_some(View { center: saved.center, height })
}
