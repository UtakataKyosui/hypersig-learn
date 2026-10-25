//! Date and time inputs bound to a form field, with the label, error region
//! and ARIA wiring `hypersig_ui::TextField::in_form` gives a text field.
//!
//! `hypersig_ui` 0.4.1 has no date or time kind, so these wrap a native
//! `<input type="date">` / `<input type="time">`. The form is `novalidate`:
//! a browser may hand back any string, so `Validate` rules and the server's
//! parse decide validity.
//!
//! A decimal amount uses `TextField::in_form(..).input_mode(InputMode::Decimal)`.

use hypersig::html::{El, div, input, label, p, span};
use hypersig::style::Style;
use hypersig::style::pattern::stack;
use hypersig::style::value::{BorderStyle, Color, FontWeight, LineHeight, MinWidth, pct, px, rem};
use hypersig::{Form, Sig, Validate, if_else};
use hypersig_ui::token;

/// Which native picker a [`temporal_field`] shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Temporal {
    /// `type="date"`, value `YYYY-MM-DD`.
    Date,
    /// `type="time"`, value `HH:MM`.
    Time,
}

impl Temporal {
    const fn input_type(self) -> &'static str {
        match self {
            Self::Date => "date",
            Self::Time => "time",
        }
    }
}

/// A labelled date input bound to `field` of `form`.
#[must_use]
pub fn date_field<S, T: Validate>(
    label: &str,
    form: Sig<S, Form<T>>,
    field: Sig<T, String>,
) -> El<S> {
    temporal_field(Temporal::Date, label, form, field, false)
}

/// A labelled time input bound to `field` of `form`.
#[must_use]
pub fn time_field<S, T: Validate>(
    label: &str,
    form: Sig<S, Form<T>>,
    field: Sig<T, String>,
) -> El<S> {
    temporal_field(Temporal::Time, label, form, field, false)
}

/// A labelled date or time input bound to `field` of `form`.
///
/// The control has id `ui-<path>-input` (path of `form.field(field)`, e.g. `pause.value.start`, with
/// `.` as `-`: the same id `TextField` derives), `aria-describedby` pointing at
/// the `aria-live="polite"` error region `ui-<path>-error`, and `aria-invalid`
/// true while the form's message for `field` is not empty. Leaving the control
/// marks `field` touched. `required` adds the `required` attribute and a
/// visual asterisk only; emptiness is still judged by the form's rules.
#[must_use]
pub fn temporal_field<S, T: Validate>(
    kind: Temporal,
    caption: &str,
    form: Sig<S, Form<T>>,
    field: Sig<T, String>,
    required: bool,
) -> El<S> {
    let value = form.field(field);
    let base = format!("ui-{}-", value.path().replace('.', "-"));
    let field_id = format!("{base}input");
    let error_id = format!("{base}error");
    let error = form.error(field);
    let invalid = !error.clone().is_empty();

    let mut control = input()
        .attr("type", kind.input_type())
        .css(control_style())
        .bind(value)
        .id(field_id.clone())
        .attr("aria-describedby", error_id.clone())
        .bind_attr("aria-invalid", if_else(invalid.clone(), "true", "false"))
        .css_when(
            Style::new().border_color(token::destructive().var()),
            invalid,
        )
        .on_blur(form.touch(field));
    if required {
        control = control.attr("required", "");
    }

    let mut caption_el = label()
        .attr("for", field_id)
        .css(caption_style())
        .text(caption);
    if required {
        caption_el = caption_el.child(
            span()
                .attr("aria-hidden", "true")
                .css(Style::new().color(token::destructive().var()))
                .text(" *"),
        );
    }

    div()
        .css(stack(rem(0.5)))
        .child(caption_el)
        .child(control)
        .child(
            p().id(error_id)
                .attr("aria-live", "polite")
                .css(
                    Style::new()
                        .font_size(rem(0.875))
                        .line_height(LineHeight::Length(rem(1.25)))
                        .color(token::destructive().var()),
                )
                .bind_text(error),
        )
}

fn caption_style() -> Style {
    Style::new()
        .font_size(rem(0.875))
        .line_height(LineHeight::Length(rem(1.25)))
        .font_weight(FontWeight::Number(500.0))
}

fn control_style() -> Style {
    Style::new()
        .width(pct(100))
        .min_width(MinWidth::from(px(0)))
        .height(rem(2.25))
        .padding_block(rem(0.25))
        .padding_inline(rem(0.75))
        .border_width(px(1))
        .border_style(BorderStyle::Solid)
        .border_color(token::input().var())
        .border_radius(px(6))
        .background_color(Color::TRANSPARENT)
        .color(token::foreground().var())
        .font_size(rem(0.875))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::medication_input::{DoseSlotInput, PauseInput};
    use hypersig::{FieldErrors, Signals};
    use hypersig_ui::{InputMode, TextField};
    use serde::{Deserialize, Serialize};

    #[derive(Signals, Serialize, Deserialize)]
    struct Page {
        dose: Form<DoseSlotInput>,
        pause: Form<PauseInput>,
    }

    #[test]
    fn date_field_wires_label_error_and_touch() {
        let html = date_field("開始日", Page::pause(), PauseInput::start()).render();
        assert!(html.contains(r#"type="date""#), "{html}");
        assert!(
            html.contains(r#"id="ui-pause-value-start-input""#),
            "{html}"
        );
        assert!(
            html.contains(r#"for="ui-pause-value-start-input""#),
            "{html}"
        );
        assert!(
            html.contains(r#"aria-describedby="ui-pause-value-start-error""#),
            "{html}"
        );
        assert!(
            html.contains(r#"id="ui-pause-value-start-error""#),
            "{html}"
        );
        assert!(html.contains(r#"aria-live="polite""#), "{html}");
        assert!(html.contains("aria-invalid"), "{html}");
        assert!(html.contains("pause.touched.start"), "{html}");
        assert!(!html.contains("required"), "{html}");
    }

    #[test]
    fn time_field_and_required_marker() {
        let html = temporal_field(
            Temporal::Time,
            "時刻",
            Page::dose(),
            DoseSlotInput::time(),
            true,
        )
        .render();
        assert!(html.contains(r#"type="time""#), "{html}");
        assert!(html.contains(r#"id="ui-dose-value-time-input""#), "{html}");
        assert!(html.contains("required"), "{html}");
        assert!(html.contains(r#"aria-hidden="true""#), "{html}");
    }

    #[test]
    fn amount_uses_decimal_text_field() {
        let html = TextField::in_form("量", Page::dose(), DoseSlotInput::amount())
            .input_mode(InputMode::Decimal)
            .render()
            .render();
        assert!(html.contains(r#"inputmode="decimal""#), "{html}");
        assert!(html.contains("dose.touched.amount"), "{html}");
        assert!(
            html.contains(r#"aria-describedby="ui-dose-value-amount-error""#),
            "{html}"
        );
    }

    fn rejected<T>(result: Result<T, FieldErrors<DoseSlotInput>>) -> FieldErrors<DoseSlotInput> {
        result.err().expect("expected a validation failure")
    }

    #[test]
    fn empty_and_invalid_values_report_on_their_own_field() {
        let mut input = DoseSlotInput {
            time: String::new(),
            amount: String::new(),
            unit: "錠".into(),
            timing: "unspecified".into(),
            note: String::new(),
        };
        let errors = rejected(Form::validate(&input));
        assert!(errors.message(DoseSlotInput::time()).is_some());
        assert!(errors.message(DoseSlotInput::amount()).is_some());

        input.time = "25:61".into();
        input.amount = "1.234".into();
        let errors = rejected(input.parse("slot".parse().unwrap()));
        assert!(errors.message(DoseSlotInput::time()).is_some());
        input.time = "08:00".into();
        let errors = rejected(input.parse("slot".parse().unwrap()));
        assert!(errors.message(DoseSlotInput::amount()).is_some());
        for bad in ["abc", "-1", "1001", "0"] {
            input.amount = bad.into();
            assert!(
                input
                    .parse("slot".parse().unwrap())
                    .unwrap_err()
                    .message(DoseSlotInput::amount())
                    .is_some(),
                "{bad}"
            );
        }
    }
}
