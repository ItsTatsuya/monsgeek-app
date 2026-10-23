//! Per-key colours: paint keys and store them as one of the keyboard's user pictures.

use super::*;

const STARTER: [u32; 10] = [
    0xffffff, 0xff3b3b, 0xff8a2a, 0xffd23f, 0x3ddc84, 0x3ddbc4, 0x3b9bff, 0x8b7bff, 0xff4fd8, 0x000000,
];

fn rgb3(hex: u32) -> [u8; 3] {
    [(hex >> 16) as u8, (hex >> 8) as u8, hex as u8]
}

fn hex(c: [u8; 3]) -> u32 {
    ((c[0] as u32) << 16) | ((c[1] as u32) << 8) | c[2] as u32
}

impl AppView {
    pub(super) fn per_key(&mut self, cx: &mut Cx) -> Div {
        // Brush: clicking or dragging across keys paints them straight away.
        self.paint_selection();
        let t = self.theme;
        let tabs = self.lighting_tabs(cx);
        let count = self.picture_count();
        let Some(p) = self.features.perkey.as_ref() else {
            return div();
        };
        let (picture, color, dirty) = (p.picture, p.color, p.dirty);
        let colors = p.colors.clone();
        let loaded = colors.is_some();
        let showing = self
            .device
            .as_ref()
            .map(|d| d.lighting)
            .filter(|l| l.effect == crate::protocol::userpic::USER_PICTURE_EFFECT)
            .map(|l| l.option);
        let busy = self.busy > 0;

        let board = self.keyboard(
            Select::Multi,
            move |k| {
                let c = colors.as_ref().map(|c| c[k.index]).unwrap_or([0; 3]);
                if c == [0, 0, 0] {
                    return Cap {
                        legend: action_label(&k.base),
                        dim: true,
                        ..Default::default()
                    };
                }
                let col: Hsla = rgb(hex(c)).into();
                Cap {
                    legend: action_label(&k.base),
                    fill: Some((1.0, col)),
                    lit: Some(col),
                    ..Default::default()
                }
            },
            cx,
        );
        let toolbar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().text_color(t.muted).child(if loaded {
                "Click or drag across keys to paint them"
            } else {
                "Reading key colours from the keyboard…"
            }))
            .child(
                self.small_button("pk-fill", "Fill all", Btn::Normal, loaded)
                    .when(loaded, |b| b.on_click(cx.listener(|this, _, _, cx| this.fill_picture(None, cx)))),
            )
            .child(
                self.small_button("pk-clear", "All off", Btn::Ghost, loaded)
                    .when(loaded, |b| b.on_click(cx.listener(|this, _, _, cx| this.fill_picture(Some([0; 3]), cx)))),
            );

        let swatch = |h: u32| {
            let c = rgb3(h);
            let active = c == color;
            div()
                .id(("pk-sw", h as usize))
                .size(px(30.))
                .rounded(px(8.))
                .bg(rgb(h))
                .border_2()
                .border_color(if active { t.text } else { t.border_strong })
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(p) = this.features.perkey.as_mut() {
                        p.color = c;
                    }
                    cx.notify();
                }))
        };
        // Two fixed rows: a wrapping row here throws off the keyboard card's width.
        let swatches = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(div().flex().gap(px(8.)).children(STARTER[..5].iter().map(|&h| swatch(h))))
            .child(div().flex().gap(px(8.)).children(STARTER[5..].iter().map(|&h| swatch(h))));
        // Fine control: step each channel.
        let channel = |i: usize, name: &'static str| {
            let v = color[i];
            let step = move |delta: i16| {
                move |this: &mut AppView, _: &ClickEvent, _: &mut Window, cx: &mut Cx| {
                    if let Some(p) = this.features.perkey.as_mut() {
                        p.color[i] = (p.color[i] as i16 + delta).clamp(0, 255) as u8;
                    }
                    cx.notify();
                }
            };
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(div().w(px(14.)).text_color(t.muted).child(name))
                .child(self.small_button(("pk-m16", i), "−16", Btn::Ghost, true).on_click(cx.listener(step(-16))))
                .child(self.small_button(("pk-m1", i), "−", Btn::Ghost, true).on_click(cx.listener(step(-1))))
                .child(
                    div()
                        .w(px(40.))
                        .text_center()
                        .font_family(NUM_FONT)
                        .text_size(px(12.5))
                        .child(v.to_string()),
                )
                .child(self.small_button(("pk-p1", i), "+", Btn::Ghost, true).on_click(cx.listener(step(1))))
                .child(self.small_button(("pk-p16", i), "+16", Btn::Ghost, true).on_click(cx.listener(step(16))))
        };
        let brush = div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(div().size(px(44.)).rounded(px(10.)).bg(rgb(hex(color))).border_1().border_color(t.border_strong))
                    .child(
                        div()
                            .font_family(NUM_FONT)
                            .text_size(px(13.))
                            .child(format!("#{:06X}", hex(color))),
                    ),
            )
            .child(swatches)
            .child(channel(0, "R"))
            .child(channel(1, "G"))
            .child(channel(2, "B"));

        let pictures = (count > 1).then(|| {
            self.segmented(
                "pk-pic",
                (0..count)
                    .map(|i| {
                        (
                            i,
                            if showing == Some(i) {
                                format!("{} · on", i + 1)
                            } else {
                                (i + 1).to_string()
                            },
                        )
                    })
                    .collect(),
                Some(picture),
                |this, v, _, cx| this.load_picture(v, cx),
                cx,
            )
            .into_any_element()
        });
        let can_save = loaded && !busy;
        let actions = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(div().flex_1().text_color(if dirty { t.warn } else { t.muted }).text_size(px(12.5)).child(
                if dirty {
                    "Unsaved changes. Switching pictures discards them."
                } else if showing == Some(picture) {
                    "The keyboard is showing this picture."
                } else {
                    "Saving switches the lighting to this picture."
                },
            ))
            .child(
                self.button("pk-revert", "Revert", Btn::Normal)
                    .when(!dirty, |b| b.opacity(0.45))
                    .when(dirty, |b| b.on_click(cx.listener(move |this, _, _, cx| this.load_picture(picture, cx)))),
            )
            .child(
                self.button("pk-save", "Save and show", Btn::Primary)
                    .when(!can_save, |b| b.opacity(0.45))
                    .when(can_save, |b| b.on_click(cx.listener(|this, _, _, cx| this.save_picture(cx)))),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .children(tabs)
            .child(self.keyboard_card(Some(toolbar.into_any_element()), board))
            .child(
                div()
                    .flex()
                    .gap(px(16.))
                    .items_start()
                    .when(self.narrow(), |d| d.flex_col())
                    .child(
                        div().flex_1().min_w(px(0.)).when(self.narrow(), |d| d.w_full()).child(self.card(
                            "Per-key colours",
                            Some("Pick a colour, then click or drag across keys. Saving stores the colours on the keyboard and switches the lighting to them."),
                            pictures,
                            actions,
                        )),
                    )
                    .child(
                        div()
                            .w(px(340.))
                            .when(self.narrow(), |d| d.w_full())
                            .flex_none()
                            .child(self.card("Brush", None, None, brush)),
                    ),
            )
    }
}
