use crate::overlay::warning_content::WarningContent;
use async_channel::Sender;
use gpui::{Context, IntoElement, Render, Window, div, prelude::*, px, rgb};

pub struct WarningOverlayWindow {
    pub content: WarningContent,
    pub dismissed: bool,
    pub events: Option<Sender<crate::input::hook::InputEvent>>,
}

impl Render for WarningOverlayWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .p_4()
            .text_color(rgb(0xf4f5f4))
            .flex()
            .flex_col()
            .when(!self.dismissed, |element| {
                element.child(
                    div()
                        .w(px(300.0))
                        .rounded_lg()
                        .bg(rgb(0x292e2b))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .p_3()
                                .flex()
                                .gap_3()
                                .child(div().text_2xl().text_color(rgb(0xe3b65f)).child("⚠"))
                                .child(
                                    div()
                                        .flex_1()
                                        .flex()
                                        .flex_col()
                                        .gap_1()
                                        .child(self.content.title())
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(rgb(0xc1c4c2))
                                                .child(self.content.message()),
                                        )
                                        .when_some(
                                            self.content.learn_more_url(),
                                            |element, url| {
                                                element.child(
                                                    div()
                                                        .id("warning-learn-more")
                                                        .mt_1()
                                                        .text_xs()
                                                        .cursor_pointer()
                                                        .child("Learn more")
                                                        .on_click(cx.listener(
                                                            move |_, _, _, cx| cx.open_url(url),
                                                        )),
                                                )
                                            },
                                        ),
                                )
                                .child(
                                    div()
                                        .id("dismiss-warning")
                                        .cursor_pointer()
                                        .child("×")
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            this.dismissed = true;
                                            if let Some(events) = &this.events {
                                                let _ = events.try_send(
                                                    crate::input::hook::InputEvent::DismissWarning,
                                                );
                                            }
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(div().h(px(2.0)).w_full().bg(rgb(0xe3b65f))),
                )
            })
    }
}
