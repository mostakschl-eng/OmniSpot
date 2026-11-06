use crate::ui::SettingsWindow;
use gpui::{Context, IntoElement, div, prelude::*, px, rgb};

fn key(
    label: &'static str,
    width: f32,
    selected: bool,
    selectable: bool,
    spotlight: bool,
    cx: &mut Context<SettingsWindow>,
) -> impl IntoElement {
    let cap = div()
        .id(label)
        .w(px(width))
        .h(px(43.0))
        .rounded_md()
        .bg(rgb(if selected { 0xb9b7ae } else { 0x3c413e }))
        .text_color(rgb(if selected { 0x181b19 } else { 0xd3d6d3 }))
        .flex()
        .items_end()
        .px_2()
        .py_1()
        .child(label);
    if selectable {
        cap.cursor_pointer()
            .on_click(cx.listener(move |this, _, _, cx| {
                if spotlight {
                    this.state.spotlight_modifier = label.to_string();
                } else {
                    this.state.modifier = label.to_string();
                }
                cx.notify();
            }))
            .into_any_element()
    } else {
        cap.opacity(0.4).into_any_element()
    }
}

pub(crate) fn modifier_key_selector(
    selected: &str,
    spotlight: bool,
    cx: &mut Context<SettingsWindow>,
) -> impl IntoElement {
    let first = [
        "CapsLock", "A", "S", "D", "F", "G", "H", "J", "K", "L", ";", "'", "Enter",
    ];
    let second = [
        "LeftShift",
        "Z",
        "X",
        "C",
        "V",
        "B",
        "N",
        "M",
        ",",
        ".",
        "/",
        "RightShift",
    ];
    let third = [
        "LeftCtrl",
        "LWin",
        "LeftAlt",
        "Space",
        "RightAlt",
        "Apps",
        "RightCtrl",
    ];

    let row = |labels: &[&'static str], cx: &mut Context<SettingsWindow>| {
        let mut row = div().flex().gap_1();
        for &label in labels {
            let selectable = matches!(
                label,
                "CapsLock"
                    | "LeftShift"
                    | "RightShift"
                    | "LeftCtrl"
                    | "RightCtrl"
                    | "LWin"
                    | "LeftAlt"
                    | "RightAlt"
                    | "Apps"
            );
            let width = match label {
                "Space" => 296.0,
                "CapsLock" | "Enter" | "LeftShift" | "RightShift" => 80.0,
                _ => 37.0,
            };
            row = row.child(key(
                label,
                width,
                label == selected,
                selectable,
                spotlight,
                cx,
            ));
        }
        row
    };

    div()
        .w_full()
        .p_3()
        .rounded_md()
        .bg(rgb(0x303532))
        .flex()
        .flex_col()
        .gap_1()
        .child(row(&first, cx))
        .child(row(&second, cx))
        .child(row(&third, cx))
}
