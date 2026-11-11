use gpui::{prelude::FluentBuilder as _, *};

use crate::{layout::sidebar_item::NavigationItem, theme::ActiveTheme, views::SettingsModalState};

#[derive(IntoElement)]
pub(super) struct Sidebar;

impl RenderOnce for Sidebar {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let active = NavigationItem::active(cx);

        div()
            .flex()
            .flex_col()
            .w(px(200.0))
            .h_full()
            .bg(cx.theme().background())
            .px_4()
            .pb_5()
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .pt_4()
                    .gap_3()
                    .child(
                        NavigationItem::Dashboard
                            .sidebar_button(active == Some(NavigationItem::Dashboard), cx),
                    )
                    .child(
                        NavigationItem::Downloads
                            .sidebar_button(active == Some(NavigationItem::Downloads), cx),
                    ),
            )
            .child(
                NavigationItem::Settings
                    .sidebar_button(active == Some(NavigationItem::Settings), cx),
            )
    }
}

const ITEM_SIZE: Pixels = px(42.0);
const ITEM_GAP: Pixels = px(4.0);
const SELECTION_SIZE: Pixels = px(36.0);
const DOCK_MARGIN: Pixels = px(12.0);
const EDGE_HEIGHT: Pixels = px(8.0);
const REVEALED_HEIGHT: Pixels = px(70.0);

struct DockState {
    pointer_inside: bool,
    focus: FocusHandle,
}

fn observe_dock_pointer(state: Entity<DockState>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, (), window, _| {
            let motion_state = state.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                if phase != DispatchPhase::Capture {
                    return;
                }

                let height = if motion_state.read(cx).pointer_inside {
                    REVEALED_HEIGHT
                } else {
                    EDGE_HEIGHT
                };
                let inside = !cx.has_active_drag()
                    && bounds.contains(&event.position)
                    && event.position.y >= bounds.bottom() - height;
                set_pointer_inside(&motion_state, inside, cx);
            });
            window.on_mouse_event(move |_: &MouseExitEvent, phase, _, cx| {
                if phase == DispatchPhase::Capture {
                    set_pointer_inside(&state, false, cx);
                }
            });
        },
    )
    .absolute()
    .inset_0()
}

fn set_pointer_inside(state: &Entity<DockState>, inside: bool, cx: &mut App) {
    if state.read(cx).pointer_inside != inside {
        state.update(cx, |state, cx| {
            state.pointer_inside = inside;
            cx.notify();
        });
    }
}

#[derive(IntoElement)]
pub(super) struct Dock;

impl RenderOnce for Dock {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let labels = NavigationItem::ALL.map(|item| item.label(cx));
        let state = window.use_keyed_state("navigation-dock-state", cx, |_, cx| DockState {
            pointer_inside: false,
            focus: cx.focus_handle(),
        });
        let dock_state = state.read(cx);
        let focus = dock_state.focus.clone();
        let keyboard_inside =
            window.last_input_was_keyboard() && focus.contains_focused(window, cx);
        let settings_open = cx
            .try_global::<SettingsModalState>()
            .is_some_and(|state| state.open);
        let revealed = window.is_window_active()
            && !settings_open
            && (dock_state.pointer_inside || keyboard_inside);
        let reveal_spring = SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(revealed);
        let theme = cx.theme();
        let active = NavigationItem::active(cx);
        let selected_index = NavigationItem::ALL
            .iter()
            .position(|&item| Some(item) == active);
        let slot_width = ITEM_SIZE + ITEM_GAP;
        let target = slot_width * selected_index.unwrap_or_default() as f32;
        let last_slot = slot_width * (NavigationItem::ALL.len() - 1) as f32;
        let spring = SpringAnimation::new(SpringConfig::new(300.0, 34.0, 1.0)).to(target);
        let text = theme.text_color();
        let muted = theme.text_muted();
        let surface = theme.surface();
        let raised = theme.surface_hover();
        let selection = raised.blend(muted.opacity(0.14));
        let selection_inset = (ITEM_SIZE - SELECTION_SIZE) / 2.0;

        let dock = div()
            .id("navigation-dock")
            .track_focus(&focus)
            .occlude()
            .rounded_full()
            .bg(linear_gradient(
                180.0,
                linear_color_stop(raised, 0.0),
                linear_color_stop(surface, 1.0),
            ))
            .border_1()
            .border_color(theme.border())
            .shadow_md()
            .p_1()
            .child(div().relative().flex().gap(ITEM_GAP).with_spring(
                "dock-selection-position",
                spring,
                move |row, position| {
                    let position = position.clamp(px(0.0), last_slot);
                    row.when(selected_index.is_some(), |row| {
                        row.child(
                            div()
                                .absolute()
                                .top(selection_inset)
                                .left(position + selection_inset)
                                .size(SELECTION_SIZE)
                                .rounded_full()
                                .bg(selection),
                        )
                    })
                    .children(
                        NavigationItem::ALL
                            .into_iter()
                            .enumerate()
                            .map(|(index, item)| {
                                let left = slot_width * index as f32;
                                let coverage = if selected_index.is_some() {
                                    (1.0 - f32::from(position - left).abs()
                                        / f32::from(SELECTION_SIZE))
                                    .max(0.0)
                                } else {
                                    0.0
                                };
                                item.dock_button(
                                    ITEM_SIZE,
                                    muted.blend(text.opacity(coverage)),
                                    (coverage < 0.5).then_some(text),
                                    labels[index].clone(),
                                )
                            }),
                    )
                },
            ));

        div()
            .absolute()
            .inset_0()
            .overflow_hidden()
            .child(observe_dock_pointer(state))
            .child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .flex()
                    .justify_center()
                    .child(dock)
                    .with_spring("navigation-dock-reveal", reveal_spring, |layer, phase| {
                        layer
                            .bottom(phase.interpolate_clamped(-REVEALED_HEIGHT, DOCK_MARGIN))
                            .opacity(phase.interpolate_clamped(0.0, 1.0))
                    }),
            )
    }
}
