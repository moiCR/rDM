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
        }
    }

    fn ease_in_out_quad(t: f32) -> f32 {
        if t < 0.5 {
            return 2.0 * t * t;
        }

        1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
    }

    fn ease_in_out_cubic(t: f32) -> f32 {
        if t < 0.5 {
            return 4.0 * t * t * t;
        }

        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
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

impl Animatable for Hsla {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        let a = self.to_rgb();
        let b = target.to_rgb();

        Rgba {
            r: a.r.lerp(&b.r, t),
            g: a.g.lerp(&b.g, t),
            b: a.b.lerp(&b.b, t),
            a: a.a.lerp(&b.a, t),
        }
        .into()
    }
}

impl Animatable for Point<Pixels> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        point(self.x.lerp(&target.x, t), self.y.lerp(&target.y, t))
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
            _ => *target,
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
            _ => *target,
        }
    }
}

impl Animatable for Length {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Length::Definite(a), Length::Definite(b)) => Length::Definite(a.lerp(b, t)),
            _ => *target,
        }
    }
}

impl<T: Animatable> Animatable for Option<T> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        match (self, target) {
            (Some(a), Some(b)) => Some(a.lerp(b, t)),
            (None, Some(b)) => Some(b.clone()),
            (Some(a), None) => Some(a.clone()),
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
            inset: self.inset,
        }
    }
}

impl Animatable for Vec<BoxShadow> {
    fn lerp(&self, target: &Self, t: f32) -> Self {
        if self.len() != target.len() {
            return target.clone();
        }

        self.iter()
            .zip(target.iter())
            .map(|(a, b)| a.lerp(b, t))
            .collect()
    }
}

#[derive(Clone, Default)]
pub struct TransitionsConfig {
    pub bg: Option<Duration>,
    pub border_color: Option<Duration>,
    pub border_widths: Option<Duration>,
    pub corner_radii: Option<Duration>,
    pub text_color: Option<Duration>,
    pub font_size: Option<Duration>,
    pub opacity: Option<Duration>,
    pub size: Option<Duration>,
    pub min_size: Option<Duration>,
    pub max_size: Option<Duration>,
    pub padding: Option<Duration>,
    pub margin: Option<Duration>,
    pub inset: Option<Duration>,
    pub gap: Option<Duration>,
    pub box_shadow: Option<Duration>,
    pub easing: Easing,
}

impl TransitionsConfig {
    pub fn bg(mut self, duration: Duration) -> Self {
        self.bg = Some(duration);
        self
    }

    pub fn border_color(mut self, duration: Duration) -> Self {
        self.border_color = Some(duration);
        self
    }

    pub fn border_widths(mut self, duration: Duration) -> Self {
        self.border_widths = Some(duration);
        self
    }

    pub fn corner_radii(mut self, duration: Duration) -> Self {
        self.corner_radii = Some(duration);
        self
    }

    pub fn text_color(mut self, duration: Duration) -> Self {
        self.text_color = Some(duration);
        self
    }

    pub fn font_size(mut self, duration: Duration) -> Self {
        self.font_size = Some(duration);
        self
    }

    pub fn opacity(mut self, duration: Duration) -> Self {
        self.opacity = Some(duration);
        self
    }

    pub fn size(mut self, duration: Duration) -> Self {
        self.size = Some(duration);
        self
    }

    pub fn min_size(mut self, duration: Duration) -> Self {
        self.min_size = Some(duration);
        self
    }

    pub fn max_size(mut self, duration: Duration) -> Self {
        self.max_size = Some(duration);
        self
    }

    pub fn padding(mut self, duration: Duration) -> Self {
        self.padding = Some(duration);
        self
    }

    pub fn margin(mut self, duration: Duration) -> Self {
        self.margin = Some(duration);
        self
    }

    pub fn inset(mut self, duration: Duration) -> Self {
        self.inset = Some(duration);
        self
    }

    pub fn gap(mut self, duration: Duration) -> Self {
        self.gap = Some(duration);
        self
    }

    pub fn box_shadow(mut self, duration: Duration) -> Self {
        self.box_shadow = Some(duration);
        self
    }

    pub fn colors(mut self, duration: Duration) -> Self {
        self.bg = Some(duration);
        self.border_color = Some(duration);
        self.text_color = Some(duration);
        self
    }

    pub fn geometry(mut self, duration: Duration) -> Self {
        self.size = Some(duration);
        self.min_size = Some(duration);
        self.max_size = Some(duration);
        self.padding = Some(duration);
        self.margin = Some(duration);
        self.inset = Some(duration);
        self.gap = Some(duration);
        self.border_widths = Some(duration);
        self.corner_radii = Some(duration);
        self
    }

    pub fn all(mut self, duration: Duration) -> Self {
        self = self.colors(duration).geometry(duration);
        self.font_size = Some(duration);
        self.opacity = Some(duration);
        self.box_shadow = Some(duration);
        self
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }
}

#[derive(Clone)]
struct AnimatedTrack<T: Animatable> {
    current: T,
    start: T,
    target: T,
    started_at: Option<Instant>,
}

impl<T: Animatable> AnimatedTrack<T> {
    fn new(initial: T) -> Self {
        Self {
            current: initial.clone(),
            start: initial.clone(),
            target: initial,
            started_at: None,
        }
    }

    fn step(&mut self, new_target: T, duration: Duration, easing: Easing) -> bool {
        if self.target != new_target {
            self.start = self.current.clone();
            self.target = new_target;
            self.started_at = Some(Instant::now());
        }

        let Some(started) = self.started_at else {
            return false;
        };

        let elapsed = started.elapsed().as_secs_f32();
        let total = duration.as_secs_f32().max(0.0001);
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
}

#[derive(Clone, Default)]
struct ResolvedStyle {
    bg: Option<Hsla>,
    border_color: Option<Hsla>,
    border_widths: EdgesRefinement<AbsoluteLength>,
    corner_radii: CornersRefinement<AbsoluteLength>,
    text_color: Option<Hsla>,
    font_size: Option<AbsoluteLength>,
    opacity: Option<f32>,
    size: SizeRefinement<Length>,
    min_size: SizeRefinement<Length>,
    max_size: SizeRefinement<Length>,
    padding: EdgesRefinement<DefiniteLength>,
    margin: EdgesRefinement<Length>,
    inset: EdgesRefinement<Length>,
    gap: SizeRefinement<DefiniteLength>,
    box_shadow: Vec<BoxShadow>,
}

impl ResolvedStyle {
    fn from_refinement(style: &StyleRefinement) -> Self {
        let bg = style
            .background
            .as_ref()
            .and_then(Fill::color)
            .and_then(|b| b.as_solid());

        Self {
            bg,
            border_color: style.border_color,
            border_widths: style.border_widths.clone(),
            corner_radii: style.corner_radii.clone(),
            text_color: style.text.color,
            font_size: style.text.font_size,
            opacity: style.opacity,
            size: style.size.clone(),
            min_size: style.min_size.clone(),
            max_size: style.max_size.clone(),
            padding: style.padding.clone(),
            margin: style.margin.clone(),
            inset: style.inset.clone(),
            gap: style.gap.clone(),
            box_shadow: style.box_shadow.clone().unwrap_or_default(),
        }
    }

    fn apply_hover(&mut self, hover: &StyleRefinement) {
        if let Some(bg) = hover
            .background
            .as_ref()
            .and_then(Fill::color)
            .and_then(|b| b.as_solid())
        {
            self.bg = Some(bg);
        }

        if hover.border_color.is_some() {
            self.border_color = hover.border_color;
        }

        if hover.text.color.is_some() {
            self.text_color = hover.text.color;
        }

        if hover.text.font_size.is_some() {
            self.font_size = hover.text.font_size;
        }

        if hover.opacity.is_some() {
            self.opacity = hover.opacity;
        }

        if let Some(hover_shadow) = &hover.box_shadow
            && !hover_shadow.is_empty()
        {
            self.box_shadow = hover_shadow.clone();
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
struct TransitionState {
    is_hovered: bool,
    bg: Option<AnimatedTrack<Hsla>>,
    border_color: Option<AnimatedTrack<Hsla>>,
    border_widths: Option<AnimatedTrack<EdgesRefinement<AbsoluteLength>>>,
    corner_radii: Option<AnimatedTrack<CornersRefinement<AbsoluteLength>>>,
    text_color: Option<AnimatedTrack<Hsla>>,
    font_size: Option<AnimatedTrack<AbsoluteLength>>,
    opacity: Option<AnimatedTrack<f32>>,
    size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    min_size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    max_size: Option<AnimatedTrack<SizeRefinement<Length>>>,
    padding: Option<AnimatedTrack<EdgesRefinement<DefiniteLength>>>,
    margin: Option<AnimatedTrack<EdgesRefinement<Length>>>,
    inset: Option<AnimatedTrack<EdgesRefinement<Length>>>,
    gap: Option<AnimatedTrack<SizeRefinement<DefiniteLength>>>,
    box_shadow: Option<AnimatedTrack<Vec<BoxShadow>>>,
}

impl TransitionState {
    fn advance_opt<T: Animatable>(
        slot: &mut Option<AnimatedTrack<T>>,
        target: Option<T>,
        duration: Option<Duration>,
        easing: Easing,
        is_animating: &mut bool,
    ) -> Option<T> {
        let target = target?;

        let Some(duration) = duration else {
            return Some(target);
        };

        let track = slot.get_or_insert_with(|| AnimatedTrack::new(target.clone()));

        if track.step(target, duration, easing) {
            *is_animating = true;
        }

        Some(track.current.clone())
    }

    fn advance_val<T: Animatable>(
        slot: &mut Option<AnimatedTrack<T>>,
        target: T,
        duration: Option<Duration>,
        easing: Easing,
        is_animating: &mut bool,
    ) -> T {
        let Some(duration) = duration else {
            return target;
        };

        let track = slot.get_or_insert_with(|| AnimatedTrack::new(target.clone()));

        if track.step(target, duration, easing) {
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
            style.background = Some(Fill::Color(bg.into()));
        }

        if let Some(border) = resolved.border_color {
            style.border_color = Some(border);
        }

        if let Some(text) = resolved.text_color {
            style.text.color = Some(text);
        }

        if let Some(font_size) = resolved.font_size {
            style.text.font_size = Some(font_size);
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
    }
}

impl RenderOnce for TransitionDiv {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state_entity =
            window.use_keyed_state(self.id.clone(), cx, |_, _| TransitionState::default());

        let is_hovered = state_entity.read(cx).is_hovered;
        let target = self.resolve_target_style(is_hovered);
        let easing = self.config.easing;

        let mut is_animating = false;
        let mut current = ResolvedStyle::default();

        state_entity.update(cx, |state, _| {
            current.bg = TransitionState::advance_opt(
                &mut state.bg,
                target.bg,
                self.config.bg,
                easing,
                &mut is_animating,
            );

            current.border_color = TransitionState::advance_opt(
                &mut state.border_color,
                target.border_color,
                self.config.border_color,
                easing,
                &mut is_animating,
            );

            current.text_color = TransitionState::advance_opt(
                &mut state.text_color,
                target.text_color,
                self.config.text_color,
                easing,
                &mut is_animating,
            );

            current.font_size = TransitionState::advance_opt(
                &mut state.font_size,
                target.font_size,
                self.config.font_size,
                easing,
                &mut is_animating,
            );

            current.opacity = TransitionState::advance_opt(
                &mut state.opacity,
                target.opacity,
                self.config.opacity,
                easing,
                &mut is_animating,
            );

            current.box_shadow = TransitionState::advance_val(
                &mut state.box_shadow,
                target.box_shadow,
                self.config.box_shadow,
                easing,
                &mut is_animating,
            );

            current.border_widths = TransitionState::advance_val(
                &mut state.border_widths,
                target.border_widths,
                self.config.border_widths,
                easing,
                &mut is_animating,
            );

            current.corner_radii = TransitionState::advance_val(
                &mut state.corner_radii,
                target.corner_radii,
                self.config.corner_radii,
                easing,
                &mut is_animating,
            );

            current.size = TransitionState::advance_val(
                &mut state.size,
                target.size,
                self.config.size,
                easing,
                &mut is_animating,
            );

            current.min_size = TransitionState::advance_val(
                &mut state.min_size,
                target.min_size,
                self.config.min_size,
                easing,
                &mut is_animating,
            );

            current.max_size = TransitionState::advance_val(
                &mut state.max_size,
                target.max_size,
                self.config.max_size,
                easing,
                &mut is_animating,
            );

            current.padding = TransitionState::advance_val(
                &mut state.padding,
                target.padding,
                self.config.padding,
                easing,
                &mut is_animating,
            );

            current.margin = TransitionState::advance_val(
                &mut state.margin,
                target.margin,
                self.config.margin,
                easing,
                &mut is_animating,
            );

            current.inset = TransitionState::advance_val(
                &mut state.inset,
                target.inset,
                self.config.inset,
                easing,
                &mut is_animating,
            );

            current.gap = TransitionState::advance_val(
                &mut state.gap,
                target.gap,
                self.config.gap,
                easing,
                &mut is_animating,
            );
        });

        self.apply_resolved_style(current);

        if is_animating {
            window.request_animation_frame();
        }

        let state_for_hover = state_entity.clone();

        self.inner.on_hover(move |hovered, _window, cx| {
            state_for_hover.update(cx, |state, cx| {
                if state.is_hovered == *hovered {
                    return;
                }

                state.is_hovered = *hovered;
                cx.notify();
            });
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
