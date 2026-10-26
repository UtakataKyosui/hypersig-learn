//! The layout shared by the medication pages. Mobile first: one column that
//! widens to a readable measure.
use hypersig::html::{Class, El, Stylesheet, a, div, el, span};
use hypersig::style::value::{Color, px};
use hypersig::style::{Media, Theme};
use hypersig_ui::token;

/// The pages' fixed light palette: the tokens the components read, set to the
/// design's greens for a light and a dark system alike.
#[must_use]
pub fn theme() -> Theme {
    let brand = |theme: Theme| {
        theme
            .set(&token::background(), Color::hex(0x00f6_f7f2))
            .set(&token::card(), Color::hex(0x00ff_ffff))
            .set(&token::foreground(), Color::hex(0x0020_2a26))
            .set(&token::card_foreground(), Color::hex(0x0020_2a26))
            .set(&token::primary(), Color::hex(0x0024_6b52))
            .set(&token::primary_foreground(), Color::hex(0x00ff_ffff))
            .set(&token::secondary(), Color::hex(0x00e7_efe5))
            .set(&token::secondary_foreground(), Color::hex(0x0024_6b52))
            .set(&token::muted(), Color::hex(0x00e7_efe5))
            .set(&token::muted_foreground(), Color::hex(0x005b_6860))
            .set(&token::accent(), Color::hex(0x00e7_efe5))
            .set(&token::accent_foreground(), Color::hex(0x0024_6b52))
            .set(&token::border(), Color::hex(0x00dd_e5da))
            .set(&token::input(), Color::hex(0x0087_9488))
            .set(&token::ring(), Color::hex(0x0024_6b52))
    };
    let radius = Theme::new().set(&token::radius(), px(12));
    brand(Theme::new())
        .merge(radius)
        .on(Media::Dark, brand(Theme::new()))
}

pub const STYLE: Stylesheet = Stylesheet::new(
    "
:root { color-scheme: light; }
.med-shell { max-width: 30rem; margin: 0 auto; padding: 1.5rem 1.5rem 7.5rem; display: grid; gap: 1.5rem; }
.med-bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: .75rem; }
.med-head { display: grid; gap: .25rem; }
.med-brand { color: var(--primary); font-size: 1.125rem; font-weight: 700; margin: 0; }
.med-title { font-size: 1.75rem; font-weight: 700; line-height: 1.25; margin: 0; }
.med-sub { color: var(--muted-foreground); font-size: .8125rem; margin: 0; }
.med-section { display: grid; gap: .75rem; }
.med-section-head { display: flex; align-items: baseline; justify-content: space-between; gap: .75rem; }
.med-section-title { font-size: 1.125rem; font-weight: 700; margin: 0; }
.med-hero { display: grid; gap: .25rem; padding: 1.25rem; border-radius: 1.375rem; background: #246B52; color: #fff; }
.med-hero-top { display: flex; align-items: center; justify-content: space-between; gap: .75rem; font-size: .8125rem; font-weight: 500; }
.med-chip { display: inline-flex; align-items: center; min-height: 1.875rem; padding-inline: .75rem; border-radius: .9375rem; background: #3F7F66; font-size: .75rem; font-weight: 500; }
.med-hero-time { font-size: 2.5rem; font-weight: 600; line-height: 1.2; font-variant-numeric: tabular-nums; margin: 0; }
.med-hero-name { font-size: 1.0625rem; font-weight: 600; margin: 0; overflow-wrap: anywhere; }
.med-hero-note { font-size: .8125rem; margin: 0; opacity: .9; }
.med-form { display: grid; gap: 1rem; }
.med-row2 { display: grid; grid-template-columns: 1fr 1fr; gap: .75rem; align-items: start; }
.med-days { display: flex; flex-wrap: wrap; gap: .5rem 1rem; border: 0; margin: 0; padding: 0; }
.med-days legend { padding: 0; margin-bottom: .5rem; font-size: .875rem; font-weight: 500; }
.med-seg { display: grid; grid-template-columns: repeat(3, 1fr); gap: .25rem; border: 0; margin: 0; padding: 0; }
.med-seg > legend, .med-seg > :not(label) { grid-column: 1 / -1; }
.med-seg label { justify-content: center; text-align: center; min-height: 2.75rem; padding-inline: .25rem; border-radius: .625rem; background: #E7EFE5; font-size: .875rem; font-weight: 500; cursor: pointer; }
.med-seg label { position: relative; }
.med-seg label > span:first-child { position: absolute; opacity: 0; }
.med-seg label:has(input:checked) { background: #fff; color: #246B52; font-weight: 700; box-shadow: inset 0 0 0 2px #246B52; }
.med-seg label:has(input:focus-visible) { outline: 2px solid #246B52; outline-offset: 2px; }
.med-actions { display: flex; flex-wrap: wrap; gap: .75rem; }
.med-card { gap: 1rem; padding-block: 1.25rem; border-radius: 1rem; box-shadow: none; }
.med-card > * { padding-inline: 1.25rem; }
.med-card .med-list, .med-card .med-form, .med-card .med-pad { padding-inline: 0; }
.med-card .med-list { margin-bottom: 1rem; }
.med-list { display: grid; gap: .75rem; list-style: none; margin: 0; padding: 0; }
.med-item { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: .5rem .75rem; }
.med-entry { padding: 1rem; border: 1px solid var(--border); border-radius: 1rem; background: #fff; }
.med-dose { min-height: 5.5rem; padding: 1rem 1.25rem; border-radius: 1rem; background: #fff; }
.med-dose-time { font-size: 1.25rem; font-weight: 600; font-variant-numeric: tabular-nums; }
.med-item-text { display: grid; gap: .25rem; min-width: 0; overflow-wrap: anywhere; }
.med-muted { color: var(--muted-foreground, #6b6b74); font-size: .875rem; margin: 0; }
.med-pill { display: inline-flex; align-items: center; min-height: 1.875rem; padding-inline: .75rem; border-radius: .9375rem; background: #E7EFE5; color: #246B52; font-size: .75rem; font-weight: 500; white-space: nowrap; }
.med-status { font-size: .75rem; font-weight: 500; }
.med-status[data-state='paused'] { color: var(--primary, #1a56db); }
.med-status[data-state='disabled'], .med-status[data-state='ended'], .med-status[data-state='empty'] { color: var(--muted-foreground, #6b6b74); }
.med-link { color: inherit; font-weight: 600; }
.med-pad { padding-inline: 0; margin: 0; }
.med-panel { display: grid; gap: .75rem; padding: 1.25rem; border-radius: 1.375rem; background: #E7EFE5; }
.med-panel-title { font-size: 1.0625rem; font-weight: 700; margin: 0; }
.med-row { display: flex; justify-content: space-between; gap: .75rem; font-size: .9375rem; }
.med-row strong { font-weight: 700; }
.med-note { padding: .75rem 1rem; border-radius: .75rem; background: #E7EFE5; font-size: .75rem; margin: 0; }
.med-preview { display: flex; align-items: center; gap: .75rem; padding: 1rem; border-radius: 1rem; background: #fff; }
.med-icon { display: grid; place-items: center; flex: none; width: 2.5rem; height: 2.5rem; border-radius: .75rem; background: #E7EFE5; color: #246B52; font-weight: 700; }
.med-preview-text { display: grid; min-width: 0; }
.med-preview-text strong { font-size: .9375rem; }
.med-preview-text span { font-size: .75rem; color: var(--muted-foreground); }
.med-shell input:not([type='checkbox']):not([type='radio']), .med-shell select { min-height: 3rem; border-radius: .75rem; background: #fff; }
.med-shell form button[type='submit'], .med-cta { width: 100%; min-height: 3.25rem; border-radius: .875rem; font-weight: 700; }
push-subscribe { display: grid; gap: .75rem; }
push-subscribe button { width: 100%; min-height: 3.25rem; border: 0; border-radius: .875rem; background: #246B52; color: #fff; font: inherit; font-weight: 700; cursor: pointer; }
push-subscribe button:disabled { opacity: .6; cursor: default; }
.med-item-text > .med-pill { justify-self: start; }
.med-cta { display: inline-flex; align-items: center; justify-content: center; text-decoration: none; }
.med-tabs { position: fixed; inset: auto 0 0 0; z-index: 20; display: flex; justify-content: center; background: #fff; border-top: 1px solid var(--border); padding-bottom: env(safe-area-inset-bottom); }
.med-tabs-inner { display: grid; grid-template-columns: repeat(3, 1fr); gap: .25rem; width: 100%; max-width: 30rem; padding: .5rem 1.5rem; }
.med-tab { display: grid; justify-items: center; align-content: center; gap: .125rem; min-height: 3.375rem; border-radius: .875rem; color: var(--muted-foreground); font-size: .75rem; font-weight: 500; text-decoration: none; }
.med-tab[aria-current='page'] { background: #E7EFE5; color: #246B52; font-weight: 700; }
.med-tab-icon { font-size: 1.125rem; line-height: 1; }
@media (pointer: coarse) {
  .med-shell input:not([type='checkbox']):not([type='radio']), .med-shell select { font-size: 1rem; min-height: 2.75rem; }
  .med-shell button { min-height: 2.75rem; }
  .med-shell label:has(input[type='radio']) { min-height: 2.75rem; display: flex; align-items: center; }
}
",
);

/// The tab of the bottom bar a page belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Today,
    Medications,
    Settings,
}

/// The app name and the page title that open every page.
#[must_use]
pub fn head<S>(title: El<S>, subtitle: El<S>) -> El<S> {
    div()
        .class(Class::new("med-head"))
        .child(
            hypersig::html::p()
                .class(Class::new("med-brand"))
                .text("葉 kusuri"),
        )
        .child(title.class(Class::new("med-title")))
        .child(subtitle.class(Class::new("med-sub")))
}

/// The bottom bar: each tab is a typed route, the current one is marked.
#[must_use]
pub fn tab_bar<S>(active: Tab) -> El<S> {
    let tab = |this: Tab, icon: &'static str, label: &'static str, href: hypersig::Link<S>| {
        let link = a()
            .class(Class::new("med-tab"))
            .href(href)
            .child(
                span()
                    .class(Class::new("med-tab-icon"))
                    .attr("aria-hidden", "true")
                    .text(icon),
            )
            .child(span().text(label));
        if this == active {
            link.attr("aria-current", "page")
        } else {
            link
        }
    };
    el("nav")
        .class(Class::new("med-tabs"))
        .attr("aria-label", "メニュー")
        .child(
            div()
                .class(Class::new("med-tabs-inner"))
                .child(tab(Tab::Today, "◷", "今日", crate::INDEX.link()))
                .child(tab(
                    Tab::Medications,
                    "＋",
                    "お薬",
                    crate::NEW_MEDICATION.link(),
                ))
                .child(tab(Tab::Settings, "☷", "設定", crate::NOTIFICATIONS.link())),
        )
}

#[cfg(test)]
mod tests {
    use super::STYLE;

    /// iOS Safari zooms the page when a control under 16px takes focus, and
    /// the controls need a finger-sized height on a touch screen.
    #[test]
    fn touch_screens_get_sixteen_pixel_controls_and_finger_sized_targets() {
        let css = STYLE.css();
        assert!(css.contains("@media (pointer: coarse)"));
        assert!(css.contains("font-size: 1rem; min-height: 2.75rem;"));
        assert!(css.contains(".med-shell button { min-height: 2.75rem; }"));
    }
}
