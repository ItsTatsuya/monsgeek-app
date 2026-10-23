//! Lighting page: a look per profile and Fn layer, applied by the app while it runs.

use super::*;
use crate::layer_lighting::PROFILE_COUNT;
use crate::protocol::Lighting;

fn summary(l: &Lighting) -> String {
    if l.effect == 0 {
        return "Off".into();
    }
    let effect = EFFECTS.iter().find(|e| e.id == l.effect);
    let colour = match effect {
        Some(e) if !e.color => String::new(),
        _ if l.rainbow => " · rainbow".into(),
        _ => format!(" · #{:02X}{:02X}{:02X}", l.color[0], l.color[1], l.color[2]),
    };
    let name = effect.map_or_else(
        || {
            if l.effect == crate::protocol::userpic::USER_PICTURE_EFFECT {
                format!("Per-key picture {}", l.option + 1)
            } else {
                format!("Effect {}", l.effect)
            }
        },
        |e| e.name.to_string(),
    );
    format!("{name}{colour} · {}/4", l.brightness)
}

impl AppView {
    pub(super) fn layer_lighting_card(&self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let d = self.device.as_ref().unwrap();
        let looks = self.looks();
        let (profile, fn_layer, shown) = (d.profile, d.options.fn_index, d.lighting);
        let applied = if looks.fn_layers.get(fn_layer as usize).copied().flatten().is_some() {
            Some((true, fn_layer as usize))
        } else if looks.profiles.get(profile as usize).copied().flatten().is_some() {
            Some((false, profile as usize))
        } else {
            None
        };
        let enabled = looks.enabled;
        let toggle = self.toggle(
            "ll-on",
            enabled,
            false,
            move |this, v, _, cx| {
                let mut looks = this.looks();
                looks.enabled = v;
                this.set_looks(looks, cx);
            },
            cx,
        );
        let row = |is_fn: bool, i: usize, live: bool| {
            let look = if is_fn { looks.fn_layers[i] } else { looks.profiles[i] };
            let on = looks.enabled && applied == Some((is_fn, i));
            let swatch = div()
                .flex_none()
                .size(px(22.))
                .rounded(px(6.))
                .border_1()
                .border_color(t.border_strong)
                .map(|el| match look {
                    None => el,
                    Some(l) if l.effect == 0 => el.bg(t.surface_3),
                    Some(l) if l.rainbow => el.bg(linear_gradient(
                        135.,
                        linear_color_stop(rgb(0xff5f6d), 0.),
                        linear_color_stop(rgb(0x3b9bff), 1.),
                    )),
                    Some(l) => el.bg(rgb(((l.color[0] as u32) << 16) | ((l.color[1] as u32) << 8) | l.color[2] as u32)),
                });
            let id = if is_fn { 10 + i } else { i };
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .px(px(10.))
                .py(px(8.))
                .rounded(px(8.))
                .border_1()
                .border_color(if on { t.accent } else { transparent_black() })
                .bg(if on { t.accent_soft } else { t.surface_2 })
                .child(swatch)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(if is_fn { format!("Fn layer {}", i + 1) } else { format!("Profile {}", i + 1) })
                                .when(live, |el| el.child(div().size(px(7.)).rounded_full().bg(t.ok))),
                        )
                        .child(div().text_color(t.muted).text_size(px(12.)).child(match look {
                            Some(l) => summary(&l),
                            None if is_fn => "No change".into(),
                            None => "Keyboard's own lighting".into(),
                        })),
                )
                .child(
                    self.small_button(("ll-use", id), "Use current lighting", Btn::Normal, true)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_look_slot(is_fn, i, Some(shown), cx))),
                )
                .child(
                    self.small_button(("ll-clear", id), "Clear", Btn::Ghost, look.is_some()).when(look.is_some(), |b| {
                        b.on_click(cx.listener(move |this, _, _, cx| this.set_look_slot(is_fn, i, None, cx)))
                    }),
                )
        };
        self.card(
            "Lighting per profile and Fn layer",
            Some("Give each profile and Fn layer its own look; switching on the keyboard (Fn + G/H/J/K, Fn + Right Alt) changes the lighting to match. An Fn layer's look wins over the profile's. The keyboard can't do this itself, so it only works while Monsgeek App is open."),
            Some(toggle.into_any_element()),
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .when(!enabled, |el| el.opacity(0.6))
                .child(self.section_label("Profiles"))
                .children((0..PROFILE_COUNT).map(|i| row(false, i, i == profile as usize)))
                .child(div().pt(px(8.)).child(self.section_label("Fn layers")))
                .children((0..looks.fn_layers.len()).map(|i| row(true, i, i == fn_layer as usize)))
                .child(div().pt(px(6.)).text_color(t.muted).text_size(px(12.5)).child(
                    "To set a look: pick the lighting above, then click \"Use current lighting\" on the profile or Fn layer it belongs to.",
                )),
        )
    }
}
