use gpui::*;
use std::time::{Duration, Instant};

pub fn millis(ms: u64) -> Duration {
    Duration::from_millis(ms)
}

#[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum Easing {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    #[default]
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
}

impl Easing {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOutQuad => Self::ease_in_out_quad(t),
            Easing::EaseInCubic => t * t * t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOutCubic => Self::ease_in_out_cubic(t),
            Easing::EaseInQuart => t * t * t * t,
            Easing::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            Easing::EaseInOutQuart => Self::ease_in_out_quart(t),
            Easing::EaseInExpo => {
                if t == 0.0 {
                    0.0
                } else {
                    2.0f32.powf(10.0 * t - 10.0)
                }
            }
            Easing::EaseOutExpo => {
                if t == 1.0 {
                    1.0
                } else {
                    1.0 - 2.0f32.powf(-10.0 * t)
                }
            }
            Easing::EaseInOutExpo => Self::ease_in_out_expo(t),
        }
    }

    fn ease_in_out_quad(t: f32) -> f32 {
        if t < 0.5 {
            2.0 * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
        }
    }

    fn ease_in_out_cubic(t: f32) -> f32 {
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    }

    fn ease_in_out_quart(t: f32) -> f32 {
        if t < 0.5 {
            8.0 * t * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
        }
    }

    fn ease_in_out_expo(t: f32) -> f32 {
        if t == 0.0 {
            0.0
        } else if t == 1.0 {
            1.0
        } else if t < 0.5 {
            2.0f32.powf(20.0 * t - 10.0) / 2.0
        } else {
            (2.0 - 2.0f32.powf(-20.0 * t + 10.0)) / 2.0
        }
    }
}

pub trait Animatable: Clone + std::fmt::Debug + PartialEq {
    fn lerp(&self, target: &Self, t: f32) -> Self;
}

impl Animatable for f32 {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        self + (target - self) * t
    }
}

impl Animatable for Pixels {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        *self + (*target - *self) * t
    }
}

impl Animatable for Rems {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        *self + (*target - *self) * t
    }
}

impl Animatable for Rgba {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Rgba {
            r: self.r.lerp(&target.r, t),
            g: self.g.lerp(&target.g, t),
            b: self.b.lerp(&target.b, t),
            a: self.a.lerp(&target.a, t),
        }
    }
}

impl Animatable for Hsla {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let a = self.to_rgb();
        let b = target.to_rgb();
        a.lerp(&b, t).into()
    }
}

impl Animatable for Point<Pixels> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        point(self.x.lerp(&target.x, t), self.y.lerp(&target.y, t))
    }
}

impl Animatable for Point<DefiniteLength> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Point {
            x: self.x.lerp(&target.x, t),
            y: self.y.lerp(&target.y, t),
        }
    }
}

impl Animatable for AbsoluteLength {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (AbsoluteLength::Pixels(a), AbsoluteLength::Pixels(b)) => {
                AbsoluteLength::Pixels(a.lerp(b, t))
            }
            (AbsoluteLength::Rems(a), AbsoluteLength::Rems(b)) => {
                AbsoluteLength::Rems(a.lerp(b, t))
            }
            (AbsoluteLength::Pixels(a), AbsoluteLength::Rems(b)) => {
                let target_px = px(b.0 * 16.0);
                let current_px = a.lerp(&target_px, t);
                if t >= 1.0 {
                    *target
                } else {
                    AbsoluteLength::Pixels(current_px)
                }
            }
            (AbsoluteLength::Rems(a), AbsoluteLength::Pixels(b)) => {
                let self_px = px(a.0 * 16.0);
                let current_px = self_px.lerp(b, t);
                if t >= 1.0 {
                    *target
                } else {
                    AbsoluteLength::Pixels(current_px)
                }
            }
        }
    }
}

impl Animatable for DefiniteLength {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (DefiniteLength::Absolute(a), DefiniteLength::Absolute(b)) => {
                DefiniteLength::Absolute(a.lerp(b, t))
            }
            (DefiniteLength::Fraction(a), DefiniteLength::Fraction(b)) => {
                DefiniteLength::Fraction(a.lerp(b, t))
            }
            _ => {
                if t >= 1.0 {
                    *target
                } else {
                    *self
                }
            }
        }
    }
}

impl Animatable for Length {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Length::Definite(a), Length::Definite(b)) => Length::Definite(a.lerp(b, t)),
            _ => {
                if t >= 1.0 {
                    *target
                } else {
                    *self
                }
            }
        }
    }
}

impl Animatable for FontWeight {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        FontWeight(self.0.lerp(&target.0, t))
    }
}

impl Animatable for UnderlineStyle {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        UnderlineStyle {
            thickness: self.thickness.lerp(&target.thickness, t),
            color: self.color.lerp(&target.color, t),
            wavy: if t >= 0.5 { target.wavy } else { self.wavy },
        }
    }
}

impl Animatable for StrikethroughStyle {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        StrikethroughStyle {
            thickness: self.thickness.lerp(&target.thickness, t),
            color: self.color.lerp(&target.color, t),
        }
    }
}

impl Animatable for Background {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        if let (Some(solid_self), Some(solid_target)) = (self.as_solid(), target.as_solid()) {
            solid_self.lerp(&solid_target, t).into()
        } else if t >= 1.0 {
            *target
        } else {
            *self
        }
    }
}

impl Animatable for Fill {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let Fill::Color(bg_self) = self;
        let Fill::Color(bg_target) = target;
        Fill::Color(bg_self.lerp(bg_target, t))
    }
}

impl<T: Animatable> Animatable for Option<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Some(a), Some(b)) => Some(a.lerp(b, t)),
            (None, Some(b)) => {
                if t >= 1.0 {
                    Some(b.clone())
                } else {
                    None
                }
            }
            (Some(a), None) => {
                if t >= 1.0 {
                    None
                } else {
                    Some(a.clone())
                }
            }
            (None, None) => None,
        }
    }
}

impl<T: Animatable + Default> Animatable for Size<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Size {
            width: self.width.lerp(&target.width, t),
            height: self.height.lerp(&target.height, t),
        }
    }
}

impl<T: Animatable + Default> Animatable for Edges<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Edges {
            top: self.top.lerp(&target.top, t),
            right: self.right.lerp(&target.right, t),
            bottom: self.bottom.lerp(&target.bottom, t),
            left: self.left.lerp(&target.left, t),
        }
    }
}

impl<T: Animatable + Default> Animatable for Corners<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        Corners {
            top_left: self.top_left.lerp(&target.top_left, t),
            top_right: self.top_right.lerp(&target.top_right, t),
            bottom_right: self.bottom_right.lerp(&target.bottom_right, t),
            bottom_left: self.bottom_left.lerp(&target.bottom_left, t),
        }
    }
}

impl<T: Animatable + Default> Animatable for EdgesRefinement<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        EdgesRefinement {
            top: self.top.lerp(&target.top, t),
            right: self.right.lerp(&target.right, t),
            bottom: self.bottom.lerp(&target.bottom, t),
            left: self.left.lerp(&target.left, t),
        }
    }
}

impl<T: Animatable + Default> Animatable for CornersRefinement<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        CornersRefinement {
            top_left: self.top_left.lerp(&target.top_left, t),
            top_right: self.top_right.lerp(&target.top_right, t),
            bottom_right: self.bottom_right.lerp(&target.bottom_right, t),
            bottom_left: self.bottom_left.lerp(&target.bottom_left, t),
        }
    }
}

impl<T: Animatable + Default> Animatable for SizeRefinement<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        SizeRefinement {
            width: self.width.lerp(&target.width, t),
            height: self.height.lerp(&target.height, t),
        }
    }
}

impl Animatable for BoxShadow {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        BoxShadow {
            color: self.color.lerp(&target.color, t),
            offset: self.offset.lerp(&target.offset, t),
            blur_radius: self.blur_radius.lerp(&target.blur_radius, t),
            spread_radius: self.spread_radius.lerp(&target.spread_radius, t),
            inset: if t >= 0.5 { target.inset } else { self.inset },
        }
    }
}

impl Animatable for Vec<BoxShadow> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let max_len = self.len().max(target.len());
        if max_len == 0 {
            return Vec::new();
        }

        let mut result = Vec::with_capacity(max_len);
        for i in 0..max_len {
            let a = match self.get(i) {
                Some(s) => s.clone(),
                None => {
                    let target_shadow = &target[i];
                    BoxShadow {
                        color: target_shadow.color.opacity(0.0),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(0.0),
                        inset: target_shadow.inset,
                    }
                }
            };
            let b = match target.get(i) {
                Some(s) => s.clone(),
                None => {
                    let self_shadow = &self[i];
                    BoxShadow {
                        color: self_shadow.color.opacity(0.0),
                        offset: point(px(0.0), px(0.0)),
                        blur_radius: px(0.0),
                        spread_radius: px(0.0),
                        inset: self_shadow.inset,
                    }
                }
            };
            result.push(a.lerp(&b, t));
        }

        if t >= 1.0 && self.len() > target.len() {
            result.truncate(target.len());
        }

        result
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TransitionSpec {
    pub duration: Duration,
    pub easing: Option<Easing>,
}

impl TransitionSpec {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            easing: None,
        }
    }

    pub fn with_easing(duration: Duration, easing: Easing) -> Self {
        Self {
            duration,
            easing: Some(easing),
        }
    }
}

impl From<Duration> for TransitionSpec {
    fn from(duration: Duration) -> Self {
        Self::new(duration)
    }
}

impl From<(Duration, Easing)> for TransitionSpec {
    fn from((duration, easing): (Duration, Easing)) -> Self {
        Self::with_easing(duration, easing)
    }
}

impl From<(Easing, Duration)> for TransitionSpec {
    fn from((easing, duration): (Easing, Duration)) -> Self {
        Self::with_easing(duration, easing)
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct TransitionsConfig {
    pub bg: Option<TransitionSpec>,
    pub border_color: Option<TransitionSpec>,
    pub border_widths: Option<TransitionSpec>,
    pub corner_radii: Option<TransitionSpec>,
    pub text_color: Option<TransitionSpec>,
    pub text_bg: Option<TransitionSpec>,
    pub font_size: Option<TransitionSpec>,
    pub font_weight: Option<TransitionSpec>,
    pub line_height: Option<TransitionSpec>,
    pub underline: Option<TransitionSpec>,
    pub strikethrough: Option<TransitionSpec>,
    pub opacity: Option<TransitionSpec>,
    pub size: Option<TransitionSpec>,
    pub min_size: Option<TransitionSpec>,
    pub max_size: Option<TransitionSpec>,
    pub padding: Option<TransitionSpec>,
    pub margin: Option<TransitionSpec>,
    pub inset: Option<TransitionSpec>,
    pub gap: Option<TransitionSpec>,
    pub box_shadow: Option<TransitionSpec>,
    pub flex_grow: Option<TransitionSpec>,
    pub flex_shrink: Option<TransitionSpec>,
    pub flex_basis: Option<TransitionSpec>,
    pub aspect_ratio: Option<TransitionSpec>,
    pub scrollbar_width: Option<TransitionSpec>,
    pub easing: Easing,
}

impl TransitionsConfig {
    pub fn bg(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.bg = Some(spec.into());
        self
    }

    pub fn border_color(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.border_color = Some(spec.into());
        self
    }

    pub fn border_widths(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.border_widths = Some(spec.into());
        self
    }

    pub fn corner_radii(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.corner_radii = Some(spec.into());
        self
    }

    pub fn text_color(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.text_color = Some(spec.into());
        self
    }

    pub fn text_bg(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.text_bg = Some(spec.into());
        self
    }

    pub fn font_size(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.font_size = Some(spec.into());
        self
    }

    pub fn font_weight(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.font_weight = Some(spec.into());
        self
    }

    pub fn line_height(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.line_height = Some(spec.into());
        self
    }

    pub fn underline(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.underline = Some(spec.into());
        self
    }

    pub fn strikethrough(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.strikethrough = Some(spec.into());
        self
    }

    pub fn opacity(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.opacity = Some(spec.into());
        self
    }

    pub fn size(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.size = Some(spec.into());
        self
    }

    pub fn width(self, spec: impl Into<TransitionSpec>) -> Self {
        self.size(spec)
    }

    pub fn height(self, spec: impl Into<TransitionSpec>) -> Self {
        self.size(spec)
    }

    pub fn min_size(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.min_size = Some(spec.into());
        self
    }

    pub fn max_size(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.max_size = Some(spec.into());
        self
    }

    pub fn padding(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.padding = Some(spec.into());
        self
    }

    pub fn margin(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.margin = Some(spec.into());
        self
    }

    pub fn inset(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.inset = Some(spec.into());
        self
    }

    pub fn gap(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.gap = Some(spec.into());
        self
    }

    pub fn box_shadow(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.box_shadow = Some(spec.into());
        self
    }

    pub fn shadow(self, spec: impl Into<TransitionSpec>) -> Self {
        self.box_shadow(spec)
    }

    pub fn radius(self, spec: impl Into<TransitionSpec>) -> Self {
        self.corner_radii(spec)
    }

    pub fn border(self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self.border_color(s).border_widths(s)
    }

    pub fn flex_grow(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.flex_grow = Some(spec.into());
        self
    }

    pub fn flex_shrink(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.flex_shrink = Some(spec.into());
        self
    }

    pub fn flex_basis(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.flex_basis = Some(spec.into());
        self
    }

    pub fn aspect_ratio(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.aspect_ratio = Some(spec.into());
        self
    }

    pub fn scrollbar_width(mut self, spec: impl Into<TransitionSpec>) -> Self {
        self.scrollbar_width = Some(spec.into());
        self
    }

    pub fn colors(mut self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self.bg = Some(s);
        self.border_color = Some(s);
        self.text_color = Some(s);
        self.text_bg = Some(s);
        self
    }

    pub fn geometry(mut self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self.size = Some(s);
        self.min_size = Some(s);
        self.max_size = Some(s);
        self.padding = Some(s);
        self.margin = Some(s);
        self.inset = Some(s);
        self.gap = Some(s);
        self.border_widths = Some(s);
        self.corner_radii = Some(s);
        self.flex_grow = Some(s);
        self.flex_shrink = Some(s);
        self.flex_basis = Some(s);
        self.aspect_ratio = Some(s);
        self.scrollbar_width = Some(s);
        self
    }

    pub fn typography(mut self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self.font_size = Some(s);
        self.font_weight = Some(s);
        self.line_height = Some(s);
        self.text_color = Some(s);
        self.text_bg = Some(s);
        self.underline = Some(s);
        self.strikethrough = Some(s);
        self
    }

    pub fn effects(mut self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self.opacity = Some(s);
        self.box_shadow = Some(s);
        self
    }

    pub fn all(mut self, spec: impl Into<TransitionSpec>) -> Self {
        let s = spec.into();
        self = self.colors(s).geometry(s).typography(s).effects(s);
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AnimatedTrack<T: Animatable> {
    pub current: T,
    pub start: T,
    pub target: T,
    pub started_at: Option<Instant>,
}

impl<T: Animatable> AnimatedTrack<T> {
    pub fn new(initial: T) -> Self {
        Self {
            current: initial.clone(),
            start: initial.clone(),
            target: initial,
            started_at: None,
        }
    }

    pub fn step(&mut self, new_target: T, duration: Duration, easing: Easing) -> bool {
        if self.target != new_target {
            self.start = self.current.clone();
            self.target = new_target;
            if duration.is_zero() {
                self.current = self.target.clone();
                self.started_at = None;
                return false;
            }
            if self.start == self.target {
                self.started_at = None;
                return false;
            }
            self.started_at = Some(Instant::now());
        }

        let Some(started) = self.started_at else {
            return false;
        };

        if duration.is_zero() {
            self.current = self.target.clone();
            self.started_at = None;
            return false;
        }

        let total = duration.as_secs_f32();
        if total <= 0.0 {
            self.current = self.target.clone();
            self.started_at = None;
            return false;
        }

        let elapsed = started.elapsed().as_secs_f32();
        let progress = (elapsed / total).clamp(0.0, 1.0);

        if progress >= 1.0 {
            self.current = self.target.clone();
            self.started_at = None;
            return false;
        }

        let eased = easing.apply(progress);
        self.current = self.start.lerp(&self.target, eased);

        true
    }

    #[cfg(test)]
    pub fn step_at(&mut self, new_target: T, progress: f32, easing: Easing) -> bool {
        if self.target != new_target {
            self.start = self.current.clone();
            self.target = new_target;
        }
        let progress = progress.clamp(0.0, 1.0);
        if progress >= 1.0 {
            self.current = self.target.clone();
            self.started_at = None;
            return false;
        }
        let eased = easing.apply(progress);
        self.current = self.start.lerp(&self.target, eased);
        true
    }
}

#[derive(Clone, Default, Debug, PartialEq)]
pub struct ResolvedStyle {
    pub bg: Option<Fill>,
    pub border_color: Option<Hsla>,
    pub border_widths: EdgesRefinement<AbsoluteLength>,
    pub corner_radii: CornersRefinement<AbsoluteLength>,
    pub text_color: Option<Hsla>,
    pub text_bg: Option<Hsla>,
    pub font_size: Option<AbsoluteLength>,
    pub font_weight: Option<FontWeight>,
    pub line_height: Option<DefiniteLength>,
    pub underline: Option<UnderlineStyle>,
    pub strikethrough: Option<StrikethroughStyle>,
    pub opacity: Option<f32>,
    pub size: SizeRefinement<Length>,
    pub min_size: SizeRefinement<Length>,
    pub max_size: SizeRefinement<Length>,
    pub padding: EdgesRefinement<DefiniteLength>,
    pub margin: EdgesRefinement<Length>,
    pub inset: EdgesRefinement<Length>,
    pub gap: SizeRefinement<DefiniteLength>,
    pub box_shadow: Vec<BoxShadow>,
    pub flex_grow: Option<f32>,
    pub flex_shrink: Option<f32>,
    pub flex_basis: Option<Length>,
    pub aspect_ratio: Option<f32>,
    pub scrollbar_width: Option<AbsoluteLength>,
}

impl ResolvedStyle {
    pub fn from_refinement(style: &StyleRefinement) -> Self {
        Self {
            bg: style.background.clone(),
            border_color: style.border_color,
            border_widths: style.border_widths.clone(),
            corner_radii: style.corner_radii.clone(),
            text_color: style.text.color,
            text_bg: style.text.background_color,
            font_size: style.text.font_size,
            font_weight: style.text.font_weight,
            line_height: style.text.line_height,
            underline: style.text.underline,
            strikethrough: style.text.strikethrough,
            opacity: style.opacity,
            size: style.size.clone(),
            min_size: style.min_size.clone(),
            max_size: style.max_size.clone(),
            padding: style.padding.clone(),
            margin: style.margin.clone(),
            inset: style.inset.clone(),
            gap: style.gap.clone(),
            box_shadow: style.box_shadow.clone().unwrap_or_default(),
            flex_grow: style.flex_grow,
            flex_shrink: style.flex_shrink,
            flex_basis: style.flex_basis,
            aspect_ratio: style.aspect_ratio,
            scrollbar_width: style.scrollbar_width,
        }
    }

    pub fn apply_hover(&mut self, hover: &StyleRefinement) {
        if hover.background.is_some() {
            self.bg = hover.background.clone();
        }
        if hover.border_color.is_some() {
            self.border_color = hover.border_color;
        }
        if hover.text.color.is_some() {
            self.text_color = hover.text.color;
        }
        if hover.text.background_color.is_some() {
            self.text_bg = hover.text.background_color;
        }
        if hover.text.font_size.is_some() {
            self.font_size = hover.text.font_size;
        }
        if hover.text.font_weight.is_some() {
            self.font_weight = hover.text.font_weight;
        }
        if hover.text.line_height.is_some() {
            self.line_height = hover.text.line_height;
        }
        if hover.text.underline.is_some() {
            self.underline = hover.text.underline;
        }
        if hover.text.strikethrough.is_some() {
            self.strikethrough = hover.text.strikethrough;
        }
        if hover.opacity.is_some() {
            self.opacity = hover.opacity;
        }
        if let Some(hover_shadow) = &hover.box_shadow
            && !hover_shadow.is_empty()
        {
            self.box_shadow = hover_shadow.clone();
        }
        if hover.flex_grow.is_some() {
            self.flex_grow = hover.flex_grow;
        }
        if hover.flex_shrink.is_some() {
            self.flex_shrink = hover.flex_shrink;
        }
        if hover.flex_basis.is_some() {
            self.flex_basis = hover.flex_basis;
        }
        if hover.aspect_ratio.is_some() {
            self.aspect_ratio = hover.aspect_ratio;
        }
        if hover.scrollbar_width.is_some() {
            self.scrollbar_width = hover.scrollbar_width;
        }

        Self::merge_edges(&mut self.border_widths, &hover.border_widths);
        Self::merge_corners(&mut self.corner_radii, &hover.corner_radii);
        Self::merge_size(&mut self.size, &hover.size);
        Self::merge_size(&mut self.min_size, &hover.min_size);
        Self::merge_size(&mut self.max_size, &hover.max_size);
        Self::merge_edges(&mut self.padding, &hover.padding);
        Self::merge_edges(&mut self.margin, &hover.margin);
        Self::merge_edges(&mut self.inset, &hover.inset);
        Self::merge_size(&mut self.gap, &hover.gap);
    }

    fn merge_size<T: Clone + std::fmt::Debug + Default + PartialEq>(
        base: &mut SizeRefinement<T>,
        over: &SizeRefinement<T>,
    ) {
        if over.width.is_some() {
            base.width = over.width.clone();
        }
        if over.height.is_some() {
            base.height = over.height.clone();
        }
    }

    fn merge_edges<T: Clone + std::fmt::Debug + Default + PartialEq>(
        base: &mut EdgesRefinement<T>,
        over: &EdgesRefinement<T>,
    ) {
        if over.top.is_some() {
            base.top = over.top.clone();
        }
        if over.right.is_some() {
            base.right = over.right.clone();
        }
        if over.bottom.is_some() {
            base.bottom = over.bottom.clone();
        }
        if over.left.is_some() {
            base.left = over.left.clone();
        }
    }

    fn merge_corners<T: Clone + std::fmt::Debug + Default + PartialEq>(
        base: &mut CornersRefinement<T>,
        over: &CornersRefinement<T>,
    ) {
        if over.top_left.is_some() {
            base.top_left = over.top_left.clone();
        }
        if over.top_right.is_some() {
            base.top_right = over.top_right.clone();
        }
        if over.bottom_right.is_some() {
            base.bottom_right = over.bottom_right.clone();
        }
        if over.bottom_left.is_some() {
            base.bottom_left = over.bottom_left.clone();
        }
    }
}

#[derive(Clone, Default)]
pub struct TransitionState {
    pub is_hovered: bool,
    pub bg: Option<AnimatedTrack<Fill>>,
    pub border_color: Option<AnimatedTrack<Hsla>>,
    pub border_widths: Option<AnimatedTrack<EdgesRefinement<AbsoluteLength>>>,
    pub corner_radii: Option<AnimatedTrack<CornersRefinement<AbsoluteLength>>>,
    pub text_color: Option<AnimatedTrack<Hsla>>,
    pub text_bg: Option<AnimatedTrack<Hsla>>,
    pub font_size: Option<AnimatedTrack<AbsoluteLength>>,
    pub font_weight: Option<AnimatedTrack<FontWeight>>,
    pub line_height: Option<AnimatedTrack<DefiniteLength>>,
    pub underline: Option<AnimatedTrack<UnderlineStyle>>,
    pub strikethrough: Option<AnimatedTrack<StrikethroughStyle>>,
    pub opacity: Option<AnimatedTrack<f32>>,
    pub size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    pub min_size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    pub max_size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    pub padding: Option<AnimatedTrack<EdgesRefinement<DefiniteLength>>>,
    pub margin: Option<AnimatedTrack<EdgesRefinement<Length>>>,
    pub inset: Option<AnimatedTrack<EdgesRefinement<Length>>>,
    pub gap: Option<AnimatedTrack<SizeRefinement<DefiniteLength>>>,
    pub box_shadow: Option<AnimatedTrack<Vec<BoxShadow>>>,
    pub flex_grow: Option<AnimatedTrack<f32>>,
    pub flex_shrink: Option<AnimatedTrack<f32>>,
    pub flex_basis: Option<AnimatedTrack<Length>>,
    pub aspect_ratio: Option<AnimatedTrack<f32>>,
    pub scrollbar_width: Option<AnimatedTrack<AbsoluteLength>>,
}

impl TransitionState {
    fn advance_opt<T: Animatable>(
        slot: &mut Option<AnimatedTrack<T>>,
        target: Option<T>,
        spec: Option<TransitionSpec>,
        default_easing: Easing,
        is_animating: &mut bool,
    ) -> Option<T> {
        let target = target?;

        let Some(spec) = spec else {
            return Some(target);
        };

        let easing = spec.easing.unwrap_or(default_easing);
        let track = slot.get_or_insert_with(|| AnimatedTrack::new(target.clone()));

        if track.step(target, spec.duration, easing) {
            *is_animating = true;
        }

        Some(track.current.clone())
    }

    fn advance_val<T: Animatable>(
        slot: &mut Option<AnimatedTrack<T>>,
        target: T,
        spec: Option<TransitionSpec>,
        default_easing: Easing,
        is_animating: &mut bool,
    ) -> T {
        let Some(spec) = spec else {
            return target;
        };

        let easing = spec.easing.unwrap_or(default_easing);
        let track = slot.get_or_insert_with(|| AnimatedTrack::new(target.clone()));

        if track.step(target, spec.duration, easing) {
            *is_animating = true;
        }

        track.current.clone()
    }
}

#[derive(IntoElement)]
pub struct TransitionDiv {
    id: ElementId,
    inner: Stateful<Div>,
    config: TransitionsConfig,
    hover_style: Option<StyleRefinement>,
}

impl Styled for TransitionDiv {
    fn style(&mut self) -> &mut StyleRefinement {
        self.inner.style()
    }
}

impl ParentElement for TransitionDiv {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.inner.extend(elements);
    }
}

impl InteractiveElement for TransitionDiv {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.inner.interactivity()
    }
}

impl StatefulInteractiveElement for TransitionDiv {}

impl TransitionDiv {
    pub fn hover_style(mut self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self {
        self.hover_style = Some(f(StyleRefinement::default()));
        self
    }

    pub fn hover(self, f: impl FnOnce(StyleRefinement) -> StyleRefinement) -> Self {
        self.hover_style(f)
    }

    fn resolve_target_style(&mut self, is_hovered: bool) -> ResolvedStyle {
        let mut target = ResolvedStyle::from_refinement(self.inner.style());

        if !is_hovered {
            return target;
        }

        if let Some(hover) = &self.hover_style {
            target.apply_hover(hover);
        }

        target
    }

    fn apply_resolved_style(&mut self, resolved: ResolvedStyle) {
        let style = self.inner.style();

        if let Some(bg) = resolved.bg {
            style.background = Some(bg);
        }

        if let Some(border) = resolved.border_color {
            style.border_color = Some(border);
        }

        if let Some(text) = resolved.text_color {
            style.text.color = Some(text);
        }

        if let Some(text_bg) = resolved.text_bg {
            style.text.background_color = Some(text_bg);
        }

        if let Some(font_size) = resolved.font_size {
            style.text.font_size = Some(font_size);
        }

        if let Some(font_weight) = resolved.font_weight {
            style.text.font_weight = Some(font_weight);
        }

        if let Some(line_height) = resolved.line_height {
            style.text.line_height = Some(line_height);
        }

        if let Some(underline) = resolved.underline {
            style.text.underline = Some(underline);
        }

        if let Some(strikethrough) = resolved.strikethrough {
            style.text.strikethrough = Some(strikethrough);
        }

        if let Some(opacity) = resolved.opacity {
            style.opacity = Some(opacity);
        }

        style.box_shadow = Some(resolved.box_shadow);
        style.border_widths = resolved.border_widths;
        style.corner_radii = resolved.corner_radii;
        style.size = resolved.size;
        style.min_size = resolved.min_size;
        style.max_size = resolved.max_size;
        style.padding = resolved.padding;
        style.margin = resolved.margin;
        style.inset = resolved.inset;
        style.gap = resolved.gap;

        if let Some(flex_grow) = resolved.flex_grow {
            style.flex_grow = Some(flex_grow);
        }

        if let Some(flex_shrink) = resolved.flex_shrink {
            style.flex_shrink = Some(flex_shrink);
        }

        if let Some(flex_basis) = resolved.flex_basis {
            style.flex_basis = Some(flex_basis);
        }

        if let Some(aspect_ratio) = resolved.aspect_ratio {
            style.aspect_ratio = Some(aspect_ratio);
        }

        if let Some(scrollbar_width) = resolved.scrollbar_width {
            style.scrollbar_width = Some(scrollbar_width);
        }
    }
}

impl RenderOnce for TransitionDiv {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state_entity =
            window.use_keyed_state(self.id.clone(), cx, |_, _| TransitionState::default());

        let is_hovered = state_entity.read(cx).is_hovered;
        let target = self.resolve_target_style(is_hovered);
        let default_easing = self.config.easing;

        let mut is_animating = false;
        let mut current = ResolvedStyle::default();

        state_entity.update(cx, |state, _| {
            current.bg = TransitionState::advance_opt(
                &mut state.bg,
                target.bg,
                self.config.bg,
                default_easing,
                &mut is_animating,
            );

            current.border_color = TransitionState::advance_opt(
                &mut state.border_color,
                target.border_color,
                self.config.border_color,
                default_easing,
                &mut is_animating,
            );

            current.text_color = TransitionState::advance_opt(
                &mut state.text_color,
                target.text_color,
                self.config.text_color,
                default_easing,
                &mut is_animating,
            );

            current.text_bg = TransitionState::advance_opt(
                &mut state.text_bg,
                target.text_bg,
                self.config.text_bg,
                default_easing,
                &mut is_animating,
            );

            current.font_size = TransitionState::advance_opt(
                &mut state.font_size,
                target.font_size,
                self.config.font_size,
                default_easing,
                &mut is_animating,
            );

            current.font_weight = TransitionState::advance_opt(
                &mut state.font_weight,
                target.font_weight,
                self.config.font_weight,
                default_easing,
                &mut is_animating,
            );

            current.line_height = TransitionState::advance_opt(
                &mut state.line_height,
                target.line_height,
                self.config.line_height,
                default_easing,
                &mut is_animating,
            );

            current.underline = TransitionState::advance_opt(
                &mut state.underline,
                target.underline,
                self.config.underline,
                default_easing,
                &mut is_animating,
            );

            current.strikethrough = TransitionState::advance_opt(
                &mut state.strikethrough,
                target.strikethrough,
                self.config.strikethrough,
                default_easing,
                &mut is_animating,
            );

            current.opacity = TransitionState::advance_opt(
                &mut state.opacity,
                target.opacity,
                self.config.opacity,
                default_easing,
                &mut is_animating,
            );

            current.box_shadow = TransitionState::advance_val(
                &mut state.box_shadow,
                target.box_shadow,
                self.config.box_shadow,
                default_easing,
                &mut is_animating,
            );

            current.border_widths = TransitionState::advance_val(
                &mut state.border_widths,
                target.border_widths,
                self.config.border_widths,
                default_easing,
                &mut is_animating,
            );

            current.corner_radii = TransitionState::advance_val(
                &mut state.corner_radii,
                target.corner_radii,
                self.config.corner_radii,
                default_easing,
                &mut is_animating,
            );

            current.size = TransitionState::advance_val(
                &mut state.size,
                target.size,
                self.config.size,
                default_easing,
                &mut is_animating,
            );

            current.min_size = TransitionState::advance_val(
                &mut state.min_size,
                target.min_size,
                self.config.min_size,
                default_easing,
                &mut is_animating,
            );

            current.max_size = TransitionState::advance_val(
                &mut state.max_size,
                target.max_size,
                self.config.max_size,
                default_easing,
                &mut is_animating,
            );

            current.padding = TransitionState::advance_val(
                &mut state.padding,
                target.padding,
                self.config.padding,
                default_easing,
                &mut is_animating,
            );

            current.margin = TransitionState::advance_val(
                &mut state.margin,
                target.margin,
                self.config.margin,
                default_easing,
                &mut is_animating,
            );

            current.inset = TransitionState::advance_val(
                &mut state.inset,
                target.inset,
                self.config.inset,
                default_easing,
                &mut is_animating,
            );

            current.gap = TransitionState::advance_val(
                &mut state.gap,
                target.gap,
                self.config.gap,
                default_easing,
                &mut is_animating,
            );

            current.flex_grow = TransitionState::advance_opt(
                &mut state.flex_grow,
                target.flex_grow,
                self.config.flex_grow,
                default_easing,
                &mut is_animating,
            );

            current.flex_shrink = TransitionState::advance_opt(
                &mut state.flex_shrink,
                target.flex_shrink,
                self.config.flex_shrink,
                default_easing,
                &mut is_animating,
            );

            current.flex_basis = TransitionState::advance_opt(
                &mut state.flex_basis,
                target.flex_basis,
                self.config.flex_basis,
                default_easing,
                &mut is_animating,
            );

            current.aspect_ratio = TransitionState::advance_opt(
                &mut state.aspect_ratio,
                target.aspect_ratio,
                self.config.aspect_ratio,
                default_easing,
                &mut is_animating,
            );

            current.scrollbar_width = TransitionState::advance_opt(
                &mut state.scrollbar_width,
                target.scrollbar_width,
                self.config.scrollbar_width,
                default_easing,
                &mut is_animating,
            );
        });

        if is_hovered && let Some(hover) = &self.hover_style {
            self.inner.style().refine(hover);
        }

        self.apply_resolved_style(current);

        if is_animating {
            window.request_animation_frame();
        }

        let state_for_hover = state_entity.clone();

        self.inner.on_hover(move |hovered, window, cx| {
            state_for_hover.update(cx, |state, cx| {
                if state.is_hovered == *hovered {
                    return;
                }

                state.is_hovered = *hovered;
                cx.notify();
            });
            window.refresh();
        })
    }
}

pub trait DivTransitionsExt {
    fn with_transitions(
        self,
        id: impl Into<ElementId>,
        f: impl FnOnce(TransitionsConfig) -> TransitionsConfig,
    ) -> TransitionDiv;
}

impl DivTransitionsExt for Stateful<Div> {
    fn with_transitions(
        self,
        id: impl Into<ElementId>,
        f: impl FnOnce(TransitionsConfig) -> TransitionsConfig,
    ) -> TransitionDiv {
        let id = id.into();
        TransitionDiv {
            id,
            inner: self,
            config: f(TransitionsConfig::default()),
            hover_style: None,
        }
    }
}

impl DivTransitionsExt for Div {
    fn with_transitions(
        self,
        id: impl Into<ElementId>,
        f: impl FnOnce(TransitionsConfig) -> TransitionsConfig,
    ) -> TransitionDiv {
        let id = id.into();
        TransitionDiv {
            id: id.clone(),
            inner: self.id(id),
            config: f(TransitionsConfig::default()),
            hover_style: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;

    #[test]
    fn test_animatable_f32() {
        assert_eq!(0.0f32.lerp(&100.0, 0.0), 0.0);
        assert_eq!(0.0f32.lerp(&100.0, 0.5), 50.0);
        assert_eq!(0.0f32.lerp(&100.0, 1.0), 100.0);
    }

    #[test]
    fn test_animatable_pixels_and_rems() {
        assert_eq!(px(10.0).lerp(&px(30.0), 0.0), px(10.0));
        assert_eq!(px(10.0).lerp(&px(30.0), 0.5), px(20.0));
        assert_eq!(px(10.0).lerp(&px(30.0), 1.0), px(30.0));

        assert_eq!(rems(1.0).lerp(&rems(3.0), 0.5), rems(2.0));
    }

    #[test]
    fn test_animatable_colors() {
        let black = Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 1.0,
        };
        let white = Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 1.0,
        };

        let at_0 = black.lerp(&white, 0.0);
        assert!((at_0.l - 0.0).abs() < 0.01);

        let at_50 = black.lerp(&white, 0.5);
        assert!((at_50.l - 0.5).abs() < 0.02);

        let at_100 = black.lerp(&white, 1.0);
        assert!((at_100.l - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_animatable_rgba() {
        let red = Rgba {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let blue = Rgba {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 0.5,
        };

        let mid = red.lerp(&blue, 0.5);
        assert_eq!(mid.r, 0.5);
        assert_eq!(mid.g, 0.0);
        assert_eq!(mid.b, 0.5);
        assert_eq!(mid.a, 0.75);
    }

    #[test]
    fn test_animatable_points() {
        let p1 = point(px(0.0), px(10.0));
        let p2 = point(px(100.0), px(50.0));
        assert_eq!(p1.lerp(&p2, 0.5), point(px(50.0), px(30.0)));
    }

    #[test]
    fn test_animatable_absolute_length() {
        let px10 = AbsoluteLength::Pixels(px(10.0));
        let px20 = AbsoluteLength::Pixels(px(20.0));
        assert_eq!(px10.lerp(&px20, 0.5), AbsoluteLength::Pixels(px(15.0)));

        let rem1 = AbsoluteLength::Rems(rems(1.0));
        let rem3 = AbsoluteLength::Rems(rems(3.0));
        assert_eq!(rem1.lerp(&rem3, 0.5), AbsoluteLength::Rems(rems(2.0)));

        let cross_mid = px10.lerp(&rem1, 0.5);
        assert_eq!(cross_mid, AbsoluteLength::Pixels(px(13.0)));
        assert_eq!(px10.lerp(&rem1, 1.0), rem1);
    }

    #[test]
    fn test_animatable_definite_length() {
        let abs1 = DefiniteLength::Absolute(AbsoluteLength::Pixels(px(10.0)));
        let abs2 = DefiniteLength::Absolute(AbsoluteLength::Pixels(px(30.0)));
        assert_eq!(
            abs1.lerp(&abs2, 0.5),
            DefiniteLength::Absolute(AbsoluteLength::Pixels(px(20.0)))
        );

        let frac1 = DefiniteLength::Fraction(0.2);
        let frac2 = DefiniteLength::Fraction(0.8);
        assert_eq!(frac1.lerp(&frac2, 0.5), DefiniteLength::Fraction(0.5));

        assert_eq!(abs1.lerp(&frac1, 0.4), abs1);
        assert_eq!(abs1.lerp(&frac1, 1.0), frac1);
    }

    #[test]
    fn test_animatable_length() {
        let def1 = Length::Definite(DefiniteLength::Fraction(0.2));
        let def2 = Length::Definite(DefiniteLength::Fraction(0.6));
        let mid = def1.lerp(&def2, 0.5);
        if let Length::Definite(DefiniteLength::Fraction(f)) = mid {
            assert!((f - 0.4).abs() < 0.001);
        } else {
            panic!("Expected Fraction");
        }

        assert_eq!(def1.lerp(&Length::Auto, 0.5), def1);
        assert_eq!(def1.lerp(&Length::Auto, 1.0), Length::Auto);
    }

    #[test]
    fn test_animatable_font_weight() {
        let normal = FontWeight::NORMAL;
        let bold = FontWeight::BOLD;
        assert_eq!(normal.lerp(&bold, 0.0), normal);
        assert_eq!(normal.lerp(&bold, 0.5), FontWeight((400.0 + 700.0) / 2.0));
        assert_eq!(normal.lerp(&bold, 1.0), bold);
    }

    #[test]
    fn test_animatable_underline_and_strikethrough() {
        let u1 = UnderlineStyle {
            thickness: px(1.0),
            color: Some(Hsla {
                h: 0.0,
                s: 0.0,
                l: 0.0,
                a: 1.0,
            }),
            wavy: false,
        };
        let u2 = UnderlineStyle {
            thickness: px(3.0),
            color: Some(Hsla {
                h: 0.0,
                s: 0.0,
                l: 1.0,
                a: 1.0,
            }),
            wavy: true,
        };
        let u_mid = u1.lerp(&u2, 0.5);
        assert_eq!(u_mid.thickness, px(2.0));
        assert!(u_mid.wavy);

        let s1 = StrikethroughStyle {
            thickness: px(2.0),
            color: None,
        };
        let s2 = StrikethroughStyle {
            thickness: px(6.0),
            color: None,
        };
        let s_mid = s1.lerp(&s2, 0.5);
        assert_eq!(s_mid.thickness, px(4.0));
    }

    #[test]
    fn test_animatable_fill() {
        let color1 = Background::from(Hsla {
            h: 0.0,
            s: 0.0,
            l: 0.0,
            a: 1.0,
        });
        let color2 = Background::from(Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 1.0,
        });
        let fill1 = Fill::Color(color1);
        let fill2 = Fill::Color(color2);

        let fill_mid = fill1.lerp(&fill2, 0.5);
        let Fill::Color(bg) = fill_mid;
        assert!((bg.as_solid().unwrap().l - 0.5).abs() < 0.02);
    }

    #[test]
    fn test_animatable_box_shadow() {
        let s1 = BoxShadow::new(px(0.0), px(2.0), Hsla::default())
            .blur_radius(px(4.0))
            .spread_radius(px(1.0));
        let s2 = BoxShadow::new(px(10.0), px(12.0), Hsla::default())
            .blur_radius(px(8.0))
            .spread_radius(px(3.0));

        let mid = s1.lerp(&s2, 0.5);
        assert_eq!(mid.offset, point(px(5.0), px(7.0)));
        assert_eq!(mid.blur_radius, px(6.0));
        assert_eq!(mid.spread_radius, px(2.0));
    }

    #[test]
    fn test_animatable_vec_box_shadow_fade_in_and_out() {
        let empty: Vec<BoxShadow> = Vec::new();
        let target = vec![
            BoxShadow::new(px(0.0), px(4.0), Hsla::default())
                .blur_radius(px(8.0))
                .spread_radius(px(2.0)),
        ];

        let mid_in = empty.lerp(&target, 0.5);
        assert_eq!(mid_in.len(), 1);
        assert_eq!(mid_in[0].offset, point(px(0.0), px(2.0)));
        assert_eq!(mid_in[0].blur_radius, px(4.0));
        assert_eq!(mid_in[0].spread_radius, px(1.0));

        let at_100 = empty.lerp(&target, 1.0);
        assert_eq!(at_100.len(), 1);
        assert_eq!(at_100[0].blur_radius, px(8.0));

        let mid_out = target.lerp(&empty, 0.5);
        assert_eq!(mid_out.len(), 1);
        assert_eq!(mid_out[0].blur_radius, px(4.0));

        let at_0_out = target.lerp(&empty, 1.0);
        assert!(at_0_out.is_empty());
    }

    #[test]
    fn test_animatable_option() {
        let none: Option<f32> = None;
        let some10: Option<f32> = Some(10.0);
        let some50: Option<f32> = Some(50.0);

        assert_eq!(some10.lerp(&some50, 0.5), Some(30.0));
        assert_eq!(none.lerp(&some10, 0.5), None);
        assert_eq!(none.lerp(&some10, 1.0), Some(10.0));
        assert_eq!(some10.lerp(&none, 0.5), Some(10.0));
        assert_eq!(some10.lerp(&none, 1.0), None);
        assert_eq!(none.lerp(&none, 0.5), None);
    }

    #[test]
    fn test_animatable_size_edges_corners() {
        let size1 = Size {
            width: px(10.0),
            height: px(20.0),
        };
        let size2 = Size {
            width: px(30.0),
            height: px(40.0),
        };
        assert_eq!(
            size1.lerp(&size2, 0.5),
            Size {
                width: px(20.0),
                height: px(30.0),
            }
        );

        let edges1 = Edges {
            top: px(1.0),
            right: px(2.0),
            bottom: px(3.0),
            left: px(4.0),
        };
        let edges2 = Edges {
            top: px(5.0),
            right: px(6.0),
            bottom: px(7.0),
            left: px(8.0),
        };
        assert_eq!(
            edges1.lerp(&edges2, 0.5),
            Edges {
                top: px(3.0),
                right: px(4.0),
                bottom: px(5.0),
                left: px(6.0),
            }
        );

        let c1 = Corners {
            top_left: px(2.0),
            top_right: px(4.0),
            bottom_right: px(6.0),
            bottom_left: px(8.0),
        };
        let c2 = Corners {
            top_left: px(6.0),
            top_right: px(8.0),
            bottom_right: px(10.0),
            bottom_left: px(12.0),
        };
        assert_eq!(
            c1.lerp(&c2, 0.5),
            Corners {
                top_left: px(4.0),
                top_right: px(6.0),
                bottom_right: px(8.0),
                bottom_left: px(10.0),
            }
        );
    }

    #[test]
    fn test_easing_curves() {
        let easings = [
            Easing::Linear,
            Easing::EaseInQuad,
            Easing::EaseOutQuad,
            Easing::EaseInOutQuad,
            Easing::EaseInCubic,
            Easing::EaseOutCubic,
            Easing::EaseInOutCubic,
            Easing::EaseInQuart,
            Easing::EaseOutQuart,
            Easing::EaseInOutQuart,
            Easing::EaseInExpo,
            Easing::EaseOutExpo,
            Easing::EaseInOutExpo,
        ];

        for easing in easings {
            assert_eq!(easing.apply(0.0), 0.0);
            assert_eq!(easing.apply(1.0), 1.0);
            let mid = easing.apply(0.5);
            assert!((0.0..=1.0).contains(&mid));
        }
    }

    #[test]
    fn test_track_progress_and_completion() {
        let mut track = AnimatedTrack::new(0.0f32);
        assert_eq!(track.current, 0.0);

        let active = track.step_at(100.0, 0.5, Easing::Linear);
        assert!(active);
        assert_eq!(track.current, 50.0);

        let active = track.step_at(100.0, 1.0, Easing::Linear);
        assert!(!active);
        assert_eq!(track.current, 100.0);
    }

    #[test]
    fn test_track_interrupted_retargeting() {
        let mut track = AnimatedTrack::new(0.0f32);
        track.step_at(100.0, 0.4, Easing::Linear);
        assert_eq!(track.current, 40.0);

        track.step_at(200.0, 0.5, Easing::Linear);
        assert_eq!(track.start, 40.0);
        assert_eq!(track.current, 120.0);
    }

    #[test]
    fn test_track_hover_in_and_out() {
        let mut track = AnimatedTrack::new(0.0f32);

        track.step_at(100.0, 0.3, Easing::Linear);
        assert!((track.current - 30.0).abs() < 0.001);

        track.step_at(0.0, 0.5, Easing::Linear);
        assert!((track.start - 30.0).abs() < 0.001);
        assert!((track.current - 15.0).abs() < 0.001);

        track.step_at(0.0, 1.0, Easing::Linear);
        assert_eq!(track.current, 0.0);
    }

    #[test]
    fn test_zero_duration_transition() {
        let mut track = AnimatedTrack::new(10.0f32);
        let animating = track.step(50.0, Duration::ZERO, Easing::Linear);
        assert!(!animating);
        assert_eq!(track.current, 50.0);
        assert_eq!(track.target, 50.0);
    }

    #[test]
    fn test_transitions_config_builder() {
        let config = TransitionsConfig::default()
            .all(millis(200))
            .bg((millis(500), Easing::EaseInOutCubic))
            .opacity(millis(100))
            .easing(Easing::EaseOutQuad);

        assert_eq!(config.bg.unwrap().duration, millis(500));
        assert_eq!(config.bg.unwrap().easing, Some(Easing::EaseInOutCubic));
        assert_eq!(config.opacity.unwrap().duration, millis(100));
        assert_eq!(config.opacity.unwrap().easing, None);
        assert_eq!(config.text_color.unwrap().duration, millis(200));
        assert_eq!(config.easing, Easing::EaseOutQuad);
    }

    #[test]
    fn test_property_groups() {
        let c = TransitionsConfig::default().colors(millis(150));
        assert_eq!(c.bg.unwrap().duration, millis(150));
        assert_eq!(c.border_color.unwrap().duration, millis(150));
        assert_eq!(c.text_color.unwrap().duration, millis(150));
        assert_eq!(c.text_bg.unwrap().duration, millis(150));
        assert_eq!(c.opacity, None);

        let g = TransitionsConfig::default().geometry(millis(250));
        assert_eq!(g.size.unwrap().duration, millis(250));
        assert_eq!(g.padding.unwrap().duration, millis(250));
        assert_eq!(g.margin.unwrap().duration, millis(250));
        assert_eq!(g.gap.unwrap().duration, millis(250));
        assert_eq!(g.border_widths.unwrap().duration, millis(250));
        assert_eq!(g.corner_radii.unwrap().duration, millis(250));
        assert_eq!(g.flex_grow.unwrap().duration, millis(250));

        let t = TransitionsConfig::default().typography(millis(180));
        assert_eq!(t.font_size.unwrap().duration, millis(180));
        assert_eq!(t.font_weight.unwrap().duration, millis(180));
        assert_eq!(t.line_height.unwrap().duration, millis(180));

        let e = TransitionsConfig::default().effects(millis(300));
        assert_eq!(e.opacity.unwrap().duration, millis(300));
        assert_eq!(e.box_shadow.unwrap().duration, millis(300));
    }
}
