//! The alerts every medication page shows for a failed call. Input mistakes
//! are not here: they show under the field they belong to.
use axum::http::StatusCode;
use hypersig::Sig;
use hypersig::html::{Class, El, a, div, span};
use hypersig_ui::{AlertVariant, alert, alert_description, alert_title};
use std::ops::Not as _;

/// Three failures, told apart by `status`, the HTTP status of the failed call
/// (`0` when the call got no answer): a signed-out visitor (401) is offered
/// the login page, a call with no answer is a connection problem, and any
/// other refusal shows the server's message.
pub fn alerts<S>(problem: Sig<S, String>, status: Sig<S, i32>) -> El<S> {
    let signed_out = status.equals(i32::from(StatusCode::UNAUTHORIZED.as_u16()));
    let offline = problem.is_empty().not().and(status.equals(0));
    let refused = problem
        .is_empty()
        .not()
        .and(status.equals(0).not())
        .and(signed_out.clone().not());
    div()
        .class(Class::new("med-pad"))
        .child(
            alert(AlertVariant::Destructive)
                .show(signed_out)
                .child(alert_title().text("ログインの有効期限が切れました"))
                .child(
                    alert_description()
                        .child(span().text(
                            "画面の内容は残っています。別のタブでログインし直してから、もう一度操作してください。",
                        ))
                        .child(
                            a().href(crate::auth::LOGIN.link())
                                .attr("target", "_blank")
                                .attr("rel", "noopener")
                                .text("ログインし直す"),
                        ),
                ),
        )
        .child(
            alert(AlertVariant::Destructive)
                .show(offline)
                .child(alert_title().text("通信できませんでした"))
                .child(alert_description().text(
                    "接続を確認して、もう一度操作してください。画面の内容は残っています。",
                )),
        )
        .child(
            alert(AlertVariant::Destructive)
                .show(refused)
                .child(alert_title().text("操作できませんでした"))
                .child(alert_description().bind_text(problem)),
        )
}
