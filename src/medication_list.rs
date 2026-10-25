//! The medication list: one entry per medication, each linking to its edit
//! page, and a link to register a new one.
use crate::{
    AppState,
    auth::Member,
    medication_style::STYLE,
    medications::{Error, Settings, Summary, summary},
};
use hypersig::html::{Class, El, a, div, each_keyed, h1, li, main, p, span, ul};
use hypersig::{Form, Page};
use hypersig_ui::{
    ButtonSize, ButtonVariant, Level, button_style, card, card_content, card_description,
    card_header, card_title,
};

pub async fn page(state: AppState, member: Member, (): ()) -> Result<Page<Settings>, Error> {
    let now = hypersig::Timestamp::now().as_millis();
    let items = crate::repository::list(&state.db, member.owner())
        .await?
        .into_iter()
        .map(|row| summary(&row.editor()?, row.updated_at, now))
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(render(items))
}

/// The list page for `items`.
#[must_use]
pub fn render(items: Vec<Summary>) -> Page<Settings> {
    let editor = crate::medication_edit::new_editor(String::new(), String::new());
    Page::new(
        "服薬設定",
        Settings {
            form: Form::new(editor),
            items,
            problem: String::new(),
        },
    )
    .global(hypersig_ui::global())
    .theme(hypersig_ui::theme())
    .style(STYLE)
    .child(
        main()
            .class(Class::new("med-shell"))
            .child(
                div()
                    .class(Class::new("med-bar"))
                    .child(h1().text("服薬設定"))
                    .child(
                        a().href(crate::NEW_MEDICATION.link())
                            .css(button_style(ButtonVariant::Default, ButtonSize::Default))
                            .text("薬を追加"),
                    ),
            )
            .child(medications_card())
            .child(
                div()
                    .class(Class::new("med-actions"))
                    .child(a().href(crate::INDEX.link()).text("ホーム"))
                    .child(a().href(crate::NOTIFICATIONS.link()).text("通知設定")),
            )
            .child(crate::auth::sign_out_button()),
    )
}

fn medications_card() -> El<Settings> {
    card()
        .child(
            card_header()
                .child(card_title(Level::H2).text("登録済みの薬"))
                .child(card_description().text("薬ごとに服薬枠と繰り返しを設定します。")),
        )
        .child(
            card_content()
                .child(
                    p().class(Class::new("med-pad"))
                        .show(Settings::items().is_empty())
                        .text("まだ薬が登録されていません。「薬を追加」から登録してください。"),
                )
                .child(ul().class(Class::new("med-list")).child(each_keyed(
                    Settings::items(),
                    |item| item.at(Summary::id()),
                    |item| {
                        li().class(Class::new("med-item")).child(
                            span().class(Class::new("med-item-text")).child(
                                a().class(Class::new("med-link"))
                                    .href(crate::MEDICATION.link_with(item.at(Summary::id())))
                                    .bind_text(item.at(Summary::name())),
                            ),
                        )
                    },
                ))),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary_of(id: &str, name: &str) -> Summary {
        Summary {
            id: id.into(),
            name: name.into(),
            revision: 1,
            next_at: None,
        }
    }

    #[test]
    fn links_come_from_typed_routes_and_the_empty_state_is_part_of_the_page() {
        let html = render(vec![summary_of("a", "薬A")]).render();
        assert!(html.contains(crate::NEW_MEDICATION.path()), "{html}");
        assert!(html.contains("まだ薬が登録されていません"), "{html}");
        assert!(html.contains("data-hs-key"), "{html}");
        assert!(html.contains("data-hs-attr"), "{html}");
    }

    #[test]
    fn items_are_the_first_values_of_the_page() {
        let page = render(vec![summary_of("a", "薬A"), summary_of("b", "薬B")]);
        let items = page.value_of(Settings::items());
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].name, "薬B");
    }
}
