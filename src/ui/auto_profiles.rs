//! Automatic profiles: switch the keyboard's profile when an app comes to the front.

use super::*;

impl AppView {
    fn profile_picker(
        &self,
        id: impl Into<ElementId>,
        value: Option<u8>,
        none: bool,
        on: impl Fn(&mut AppView, Option<u8>, &mut Cx) + 'static,
        cx: &mut Cx,
    ) -> Div {
        let mut options: Vec<(i8, String)> = Vec::new();
        if none {
            options.push((-1, "Don't change".into()));
        }
        options.extend((0..4).map(|p| (p as i8, (p + 1).to_string())));
        self.segmented(
            id,
            options,
            Some(value.map_or(-1, |v| v as i8)),
            move |this, v, _, cx| on(this, (v >= 0).then_some(v as u8), cx),
            cx,
        )
    }

    pub(super) fn auto_profiles(&mut self, cx: &mut Cx) -> Div {
        let t = self.theme;
        let a = self.config.auto_profiles.clone();
        let profile = self.device.as_ref().map(|d| d.profile);

        let now = a.enabled.then(|| {
            let content = match &self.tools.current_app {
                Some((exe, _)) => {
                    let target = a.target(exe);
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(div().text_color(t.muted).child("Now in front:"))
                        .child(self.kbd(exe.clone()))
                        .child(div().text_color(t.muted).child("→"))
                        .child(match target {
                            None => div().text_color(t.muted).child("no change"),
                            Some(p) => div().font_weight(FontWeight::SEMIBOLD).child(format!("Profile {}", p + 1)),
                        })
                        .when(target.is_some() && target == profile, |el| el.child(div().size(px(7.)).rounded_full().bg(t.ok)))
                }
                None => div().text_color(t.muted).child("Waiting for the foreground app…"),
            };
            div().p(px(12.)).rounded(px(8.)).bg(t.surface_2).child(content)
        });
        let main = self.card(
            "Automatic profiles",
            Some("Switch the keyboard's profile when a game or app comes to the front, and back again when you leave it."),
            Some(
                self.toggle("ap-on", a.enabled, false, |this, v, _, cx| this.edit_auto(|a| a.enabled = v, cx), cx)
                    .into_any_element(),
            ),
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(self.setting_row(
                    "Everything else",
                    "Profile for apps without a rule.",
                    self.profile_picker("ap-default", a.default_profile, true, |this, v, cx| this.edit_auto(|a| a.default_profile = v, cx), cx),
                ))
                .child(self.setting_row(
                    "Keep running in the tray",
                    "Closing the window hides Monsgeek App to the tray so switching keeps working while you play.",
                    self.toggle("ap-tray", a.tray, false, |this, v, _, cx| this.edit_auto(|a| a.tray = v, cx), cx),
                ))
                .children(now),
        );

        let rules = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .when(a.rules.is_empty(), |el| {
                el.child(div().text_color(t.muted).child("No rules yet. Add a game below, or pick one you've had open."))
            })
            .children(a.rules.iter().enumerate().map(|(i, r)| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .bg(t.surface_2)
                    .child(div().flex_1().min_w(px(0.)).overflow_hidden().text_ellipsis().whitespace_nowrap().font_family(NUM_FONT).text_size(px(12.5)).child(r.exe.clone()))
                    .child(self.profile_picker(
                        ("ap-rule", i),
                        Some(r.profile),
                        false,
                        move |this, v, cx| {
                            if let Some(p) = v {
                                this.edit_auto(|a| a.rules[i].profile = p, cx)
                            }
                        },
                        cx,
                    ))
                    .child(
                        self.small_button(("ap-del", i), "✕", Btn::Ghost, true)
                            .on_click(cx.listener(move |this, _, _, cx| this.edit_auto(|a| { a.rules.remove(i); }, cx))),
                    )
            }));
        let text = self.tools.rule_text.clone();
        let add = div()
            .flex()
            .gap(px(8.))
            .child(self.text_field(Field::RuleExe, &text, "App name, e.g. VALORANT-Win64-Shipping.exe", 360., cx))
            .child(
                self.button("ap-add", "Add rule", Btn::Normal)
                    .when(text.trim().is_empty(), |b| b.opacity(0.45))
                    .when(!text.trim().is_empty(), |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            let text = this.tools.rule_text.clone();
                            this.add_rule(&text, cx)
                        }))
                    }),
            );
        let rules_card = self.card(
            "Rules",
            Some("The first matching app name wins. Names are matched without regard to case."),
            None,
            div().flex().flex_col().gap(px(14.)).child(rules).child(add),
        );

        let suggestions: Vec<(String, String)> = self
            .tools
            .recent_apps
            .iter()
            .filter(|(exe, _)| !a.rules.iter().any(|r| r.exe.eq_ignore_ascii_case(exe)))
            .cloned()
            .collect();
        let recent = if !a.enabled {
            div().text_color(t.muted).child("Turn on automatic profiles, then switch to your game for a moment and come back.")
        } else if suggestions.is_empty() {
            div().text_color(t.muted).child("Nothing new yet. Switch to the app you want a rule for, then come back.")
        } else {
            div().flex().flex_col().gap(px(6.)).children(suggestions.into_iter().enumerate().map(|(i, (exe, title))| {
                let add = exe.clone();
                div()
                    .id(("ap-recent", i))
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(10.))
                    .py(px(6.))
                    .rounded(px(8.))
                    .bg(t.surface_2)
                    .cursor_pointer()
                    .hover(|s| s.bg(t.hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.add_rule(&add, cx)))
                    .child(self.kbd(exe))
                    .child(div().flex_1().min_w(px(0.)).overflow_hidden().text_ellipsis().whitespace_nowrap().text_color(t.muted).text_size(px(12.5)).child(title))
                    .child(self.pill("+ Rule"))
            }))
        };
        let recent_card = self.card(
            "Recent apps",
            Some("Apps that came to the front while automatic profiles was on. Click one to add a rule."),
            None,
            recent,
        );

        div()
            .flex()
            .gap(px(16.))
            .items_start()
            .when(self.narrow(), |d| d.flex_col())
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .when(self.narrow(), |d| d.w_full())
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(main)
                    .child(rules_card),
            )
            .child(div().w(px(380.)).flex_none().when(self.narrow(), |d| d.w_full()).child(recent_card))
    }
}
