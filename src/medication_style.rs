//! The layout shared by the medication pages. Mobile first: one column that
//! widens to a readable measure.
use hypersig::html::Stylesheet;

pub const STYLE: Stylesheet = Stylesheet::new(
    "
.med-shell { max-width: 40rem; margin: 0 auto; padding: 1.5rem 1rem 4rem; display: grid; gap: 1.25rem; }
.med-bar { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: .75rem; }
.med-form { display: grid; gap: 1rem; padding-inline: 1.5rem; }
.med-days { display: flex; flex-wrap: wrap; gap: .5rem 1rem; border: 0; margin: 0; padding: 0; }
.med-days legend { padding: 0; margin-bottom: .5rem; font-size: .875rem; font-weight: 500; }
.med-actions { display: flex; flex-wrap: wrap; gap: .75rem; }
.med-list { display: grid; gap: .75rem; list-style: none; margin: 0; padding: 0 1.5rem; }
.med-item { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: .5rem .75rem; }
.med-item-text { display: grid; gap: .25rem; min-width: 0; overflow-wrap: anywhere; }
.med-muted { color: var(--muted-foreground, #6b6b74); font-size: .875rem; margin: 0; }
.med-status { font-size: .875rem; font-weight: 600; }
.med-status[data-state='paused'] { color: var(--primary, #1a56db); }
.med-status[data-state='disabled'], .med-status[data-state='ended'], .med-status[data-state='empty'] { color: var(--muted-foreground, #6b6b74); }
.med-link { color: inherit; font-weight: 600; }
.med-pad { padding-inline: 1.5rem; margin: 0; }
@media (pointer: coarse) {
  .med-shell input:not([type='checkbox']):not([type='radio']), .med-shell select { font-size: 1rem; min-height: 2.75rem; }
  .med-shell button { min-height: 2.75rem; }
  .med-shell label:has(input[type='radio']) { min-height: 2.75rem; display: flex; align-items: center; }
}
",
);

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
