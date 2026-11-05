//! Smooth mouse-wheel scrolling for a scroll container.
//!
//! GPUI jumps a whole wheel notch at once (three lines on Windows). [`layer`] is a canvas placed
//! as the container's first child: its wheel listener runs before the container's own, takes
//! line-based (wheel) deltas, and eases the offset toward the target over a few frames.
//! Pixel deltas (precision touchpads) are already smooth and pass through untouched.

use gpui::{
    canvas, point, px, App, Bounds, DispatchPhase, HitboxBehavior, IntoElement, Pixels,
    ScrollEvent, ScrollHandle, Styled, Window,
};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// How quickly the offset closes the gap: about 95% of the way in 3τ.
const TAU: f32 = 0.075;
/// Scroll a little further per notch than GPUI's three lines (about 60 px at this text size).
const SPEED: f32 = 1.35;

#[derive(Clone, Default)]
pub struct SmoothScroll {
    pub handle: ScrollHandle,
    state: Rc<RefCell<State>>,
}

#[derive(Default)]
struct State {
    /// Where the animation is heading (a scroll offset, ≤ 0), while it runs.
    target: Option<Pixels>,
    last_frame: Option<Instant>,
}

impl SmoothScroll {
    /// Back to the top, with no animation (e.g. on a page change).
    pub fn reset(&self) {
        self.handle.set_offset(point(px(0.), px(0.)));
        *self.state.borrow_mut() = State::default();
    }
}

/// One animation step: the new offset, and whether it has arrived.
fn step(current: f32, target: f32, dt: Duration) -> (f32, bool) {
    let dt = dt.as_secs_f32().clamp(0.0, 0.05);
    let next = current + (target - current) * (1.0 - (-dt / TAU).exp());
    if (target - next).abs() < 0.5 {
        (target, true)
    } else {
        (next, false)
    }
}

/// The invisible layer that drives smooth scrolling for `s.handle`'s container. Put it first
/// among the container's children so nested scroll areas still get their own wheel events.
pub fn layer(s: &SmoothScroll) -> impl IntoElement {
    let prepaint = s.clone();
    let paint = s.clone();
    canvas(
        move |bounds, window, _| {
            // The layer is laid out inside the scrolled content; undo the offset to get the
            // container's visible area.
            let offset = prepaint.handle.offset();
            let visible = Bounds::new(point(bounds.origin.x - offset.x, bounds.origin.y - offset.y), bounds.size);
            window.insert_hitbox(visible, HitboxBehavior::Normal)
        },
        move |_, hitbox, window, _: &mut App| {
            let s = paint.clone();
            let line_height = window.line_height();
            let wheel = s.clone();
            window.on_mouse_event(move |e: &ScrollEvent, phase, window: &mut Window, cx: &mut App| {
                if phase != DispatchPhase::Bubble || e.delta.precise() || !hitbox.should_handle_scroll(window) {
                    return;
                }
                let dy = e.delta.pixel_delta(line_height).y * SPEED;
                if dy == px(0.) {
                    return;
                }
                let max = wheel.handle.max_offset().y;
                let mut st = wheel.state.borrow_mut();
                let from = st.target.unwrap_or(wheel.handle.offset().y);
                let to = (from + dy).clamp(-max, px(0.));
                // At an end already: let the event go (nothing to scroll here).
                if to == from && st.target.is_none() {
                    return;
                }
                st.target = Some(to);
                cx.stop_propagation();
                window.refresh();
            });

            let mut st = s.state.borrow_mut();
            let Some(target) = st.target else {
                st.last_frame = None;
                return;
            };
            let now = Instant::now();
            let dt = st.last_frame.map_or(Duration::from_millis(16), |t| now - t);
            st.last_frame = Some(now);
            let current = s.handle.offset();
            let (y, done) = step(f32::from(current.y), f32::from(target), dt);
            s.handle.set_offset(point(current.x, px(y)));
            if done {
                st.target = None;
            }
            // Also after the last step, so the final offset gets drawn.
            window.request_animation_frame();
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eases_toward_the_target_and_arrives() {
        let (mut y, mut frames) = (0.0f32, 0);
        loop {
            let (next, done) = step(y, -120.0, Duration::from_millis(16));
            assert!(next <= y && next >= -120.0);
            y = next;
            frames += 1;
            if done {
                break;
            }
        }
        assert_eq!(y, -120.0);
        // Roughly a quarter to half a second at 60 fps: smooth, not sluggish.
        assert!((10..40).contains(&frames), "{frames} frames");
    }

    #[test]
    fn a_long_pause_does_not_overshoot() {
        let (y, done) = step(0.0, -100.0, Duration::from_secs(2));
        assert!(y >= -100.0);
        assert!(!done || y == -100.0);
    }
}
