//! A counter that runs on Cloudflare Workers, saves its count in D1, and
//! shows a clock the worker streams.
//!
//! The page, its actions, and its streams are the same hypersig code an axum
//! server runs. The worker hands each request to the axum `Router`.
//!
//! Run `npm run dev` (`cf dev`) and open <http://localhost:8787>. The deploy job of
//! `.github/workflows/ci.yml` publishes it. Each request may run in a
//! different isolate, so a value kept in memory does not reach the next request, and the saved
//! count lives in the D1 database that `cloudflare.config.ts` binds as `DB`. Every
//! open tab shows the saved count, and a save made through one isolate
//! reaches the tabs of the others through the `Hub` Durable Object.

mod assets;
mod auth;
mod auth_store;
pub mod domain;
mod failure;
pub mod form_fields;
mod medication_edit;
pub mod medication_form;
pub mod medication_input;
mod medication_list;
mod medication_style;
mod medications;
mod notifications;
mod oidc;
mod pause_edit;
mod push;
mod repository;
mod schedule_status;
pub mod schedule_view;
mod session_state;
mod session_store;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use futures::channel::mpsc::{self, UnboundedSender};
use futures::{Stream, StreamExt as _, future, stream};
use hypersig::html::{Class, Stylesheet, div, h1, main, p, strong};
use hypersig::{
    Action, DateFormat, DateStyle, IntlLocale, Page, Patches, Route, RouterExt, Signals,
    StreamAction, TimeStyle, Timestamp,
};
use hypersig_ui::{
    AlertVariant, ButtonSize, ButtonVariant, Level, alert, alert_description, alert_title, button,
    card, card_content, card_description, card_header, card_title,
};
use serde::{Deserialize, Serialize};
use tower_service::Service as _;
use tower_sessions::{Expiry, SessionManagerLayer, cookie::SameSite};
use worker::send::{SendFuture, SendWrapper};
use worker::{
    ByteStream, Context, D1Database, Delay, DurableObject, Env, HttpRequest, State, Stub,
    console_error, durable_object, event,
};

#[derive(Signals, Serialize, Deserialize)]
struct Counter {
    count: i32,
    saved: i32,
    now: Timestamp,
    problem: String,
}

hypersig::routes! {
    fn pages(Router<AppState>);
    const INDEX: Route = "/" => index;
    const NOTIFICATIONS: Route = "/notifications" => notifications::settings_page;
    const MEDICATIONS: Route = "/medications" => medication_list::page;
    const NEW_MEDICATION: Route = "/medications/new" => medication_edit::new_page;
    const MEDICATION: Route<String> = "/medications/{id}" => medication_edit::page;
}

const DOUBLE: Action<Counter, i32> = Action::post("/double");
const SAVE: Action<Counter, i32> = Action::post("/save");
const CLOCK: StreamAction<Counter> = StreamAction::new("/clock");
const SAVES: StreamAction<Counter> = StreamAction::new("/saves");

const TIME: DateFormat =
    DateFormat::date_time(IntlLocale::En, DateStyle::Medium, TimeStyle::Medium);

const STYLE: Stylesheet = Stylesheet::new(
    "
.shell { max-width: 44rem; margin: 0 auto; padding: clamp(2rem, 8vw, 5rem) 1rem; }
.intro { margin-bottom: 2rem; }
.eyebrow { color: var(--muted-foreground, #6b6b74); font-size: .75rem; font-weight: 600; letter-spacing: .12em; text-transform: uppercase; }
h1 { font-size: clamp(2rem, 6vw, 3rem); font-weight: 700; letter-spacing: -.04em; margin: .5rem 0; }
.muted { color: var(--muted-foreground, #6b6b74); font-size: .875rem; }
.counter-value { display: block; font-size: clamp(4rem, 16vw, 6rem); font-weight: 600; line-height: 1.2; letter-spacing: -.05em; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; margin: .5rem 0 1.5rem; }
.actions { display: flex; flex-wrap: wrap; gap: .75rem; }
.feedback { margin-top: 1rem; }
.details { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 14rem), 1fr)); gap: 1rem; margin-top: 1rem; }
.saved-value { display: block; font-size: 2rem; font-weight: 600; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; }
.clock { display: block; font-size: .875rem; font-variant-numeric: tabular-nums; }
",
);

/// The binding of the D1 database and the stub of the hub. A D1 handle is a
/// JavaScript object, which is not `Send`, and a worker runs on one thread,
/// so the wrapper never moves it to another.
#[derive(Clone)]
struct AppState {
    oidc: oidc::Config,
    db: SendWrapper<Rc<D1Database>>,
    hub: Arc<Stub>,
    /// The VAPID public key the browser subscribes with.
    vapid_public: String,
}

/// The name of the one `Hub` every isolate talks to. A name names one
/// Durable Object of the `HUB` binding, so the streams of every isolate
/// subscribe to the same hub.
const HUB: &str = "saves";

/// Tells the streams of every isolate that a count was saved. It holds no
/// count, only the subscribers, which it keeps in memory: when Cloudflare
/// evicts it, their responses end and each stream subscribes again.
#[durable_object(fetch)]
pub struct Hub {
    subscribers: RefCell<Vec<UnboundedSender<Vec<u8>>>>,
}

impl DurableObject for Hub {
    fn new(_state: State, _env: Env) -> Self {
        Self {
            subscribers: RefCell::default(),
        }
    }

    /// `/subscribe` answers with a body that receives one line per save, and
    /// `/publish` sends that line to every subscriber whose body is still
    /// being read.
    fn fetch(
        &self,
        request: worker::Request,
    ) -> impl Future<Output = worker::Result<worker::Response>> {
        std::future::ready(match request.path().as_str() {
            "/subscribe" => {
                let (sender, receiver) = mpsc::unbounded();
                self.subscribers.borrow_mut().push(sender);
                worker::Response::from_stream(receiver.map(Ok::<_, worker::Error>))
            }
            "/publish" => {
                self.subscribers
                    .borrow_mut()
                    .retain(|subscriber| subscriber.unbounded_send(b"\n".to_vec()).is_ok());
                worker::Response::empty()
            }
            _ => worker::Response::error("Not Found", 404),
        })
    }
}

/// The body of a `/subscribe` response. A JavaScript stream is not `Send`
/// either.
type Feed = SendWrapper<ByteStream>;

/// A D1 failure with a public message and a value-free log classification.
struct DbError;

impl From<worker::Error> for DbError {
    fn from(_: worker::Error) -> Self {
        Self
    }
}

impl IntoResponse for DbError {
    fn into_response(self) -> Response {
        console_error!("D1: storage failure");
        let message = "The count could not be read or saved. Try again.";
        (StatusCode::INTERNAL_SERVER_ERROR, message).into_response()
    }
}

impl AppState {
    async fn saved(&self, member: &auth::Member) -> Result<i32, DbError> {
        Ok(repository::counter_saved(&self.db, member.owner()).await?)
    }
    async fn save(&self, member: &auth::Member, count: i32) -> Result<(), DbError> {
        Ok(repository::counter_save(&self.db, member.owner(), count).await?)
    }

    /// Tells the hub that a count was saved. A failure is logged and the save
    /// stands, and the tabs of other isolates show it on their next load.
    async fn publish(&self) {
        let hub = Arc::clone(&self.hub);
        let sent = SendFuture::new(async move { hub.fetch_with_str("https://hub/publish").await });
        if let Err(error) = sent.await {
            console_error!("hub: {error}");
        }
    }

    async fn subscribe(&self) -> worker::Result<Feed> {
        let hub = Arc::clone(&self.hub);
        SendFuture::new(async move {
            let mut response = hub.fetch_with_str("https://hub/subscribe").await?;
            Ok(SendWrapper::new(response.stream()?))
        })
        .await
    }

    /// `feed` once the hub has told it of a save, or a new feed when there is
    /// none yet or the hub closed it. A save made while no feed was open is
    /// not announced again, so the caller reads the count either way.
    async fn next_save(&self, feed: Option<Feed>) -> Feed {
        if let Some(mut feed) = feed
            && let Some(Ok(_)) = SendFuture::new(feed.next()).await
        {
            return feed;
        }
        loop {
            match self.subscribe().await {
                Ok(feed) => return feed,
                Err(error) => {
                    console_error!("hub: {error}");
                    SendFuture::new(Delay::from(Duration::from_secs(1))).await;
                }
            }
        }
    }
}

async fn index(state: AppState, member: auth::Member, (): ()) -> Result<Page<Counter>, DbError> {
    let count = Counter::count();
    let problem = Counter::problem();
    let saved = state.saved(&member).await?;
    let initial = Counter {
        count: saved,
        saved,
        now: Timestamp::now(),
        problem: String::new(),
    };
    Ok(Page::new("hypersig on Cloudflare Workers", initial)
        .global(hypersig_ui::global())
        .theme(hypersig_ui::theme())
        .style(STYLE)
        .child(
            main()
                .class(Class::new("shell"))
                .on_load(CLOCK.call().reconnect())
                .child(
                    div()
                        .class(Class::new("intro"))
                        .child(p().class(Class::new("eyebrow")).text("hypersig playground"))
                        .child(h1().text("Counter"))
                        .child(
                            p().class(Class::new("muted"))
                                .text("Change a number. Save it. See it across every tab."),
                        ),
                )
                .child(
                    card()
                        .child(
                            card_header()
                                .child(card_title(Level::H2).text("Your counter"))
                                .child(
                                    card_description()
                                        .text("Add instantly, or double the value on the server."),
                                ),
                        )
                        .child(
                            card_content()
                                .child(
                                    strong()
                                        .class(Class::new("counter-value"))
                                        .bind_text(count.to_text()),
                                )
                                .child(
                                    div()
                                        .class(Class::new("actions"))
                                        .child(
                                            button(ButtonVariant::Default, ButtonSize::Lg)
                                                .on_click(count.set(count + 1))
                                                .text("+1 · Add one"),
                                        )
                                        .child(
                                            button(ButtonVariant::Secondary, ButtonSize::Lg)
                                                .on_click(DOUBLE.call_with(count))
                                                .text("×2 · Double"),
                                        )
                                        .child(
                                            button(ButtonVariant::Outline, ButtonSize::Lg)
                                                .on_click(SAVE.call_with(count).error(problem))
                                                .text("Save value"),
                                        ),
                                )
                                .child(
                                    alert(AlertVariant::Destructive)
                                        .class(Class::new("feedback"))
                                        .show(!problem.is_empty())
                                        .child(alert_title().text("Could not save"))
                                        .child(alert_description().bind_text(problem)),
                                ),
                        ),
                )
                .child(counter_details())
                .child(
                    hypersig::html::a()
                        .href(NOTIFICATIONS.link())
                        .text("通知設定"),
                )
                .child(auth::sign_out_button()),
        ))
}

fn counter_details() -> hypersig::html::El<Counter> {
    div()
        .class(Class::new("details"))
        .child(
            card()
                .on_load(SAVES.call().reconnect())
                .child(
                    card_header()
                        .child(card_title(Level::H2).text("Saved value"))
                        .child(card_description().text("Shared across all open tabs.")),
                )
                .child(
                    card_content().child(
                        strong()
                            .class(Class::new("saved-value"))
                            .bind_text(Counter::saved().to_text()),
                    ),
                ),
        )
        .child(
            card()
                .child(
                    card_header()
                        .child(card_title(Level::H2).text("Server clock"))
                        .child(card_description().text("Live updates, every second.")),
                )
                .child(
                    card_content().child(
                        p().class(Class::new("clock"))
                            .bind_text(Counter::now().format_date(&TIME)),
                    ),
                ),
        )
}

async fn double(_: AppState, _: auth::Member, count: i32) -> Patches<Counter> {
    Counter::count().patch(count.saturating_mul(2)).into()
}

async fn save(
    state: AppState,
    member: auth::Member,
    count: i32,
) -> Result<Patches<Counter>, DbError> {
    state.save(&member, count).await?;
    state.publish().await;
    Ok(Counter::problem().patch(String::new()).into())
}

/// The saved count, once, then again after each save through any isolate.
/// A failed read is logged and skipped, so the tab keeps the count it shows.
fn saves(state: AppState, member: auth::Member, (): ()) -> impl Stream<Item = Patches<Counter>> {
    stream::unfold((state, member, None), |(state, member, feed)| async move {
        let feed = state.next_save(feed).await;
        let patch = if let Ok(saved) = state.saved(&member).await {
            Some(Counter::saved().patch(saved).into())
        } else {
            console_error!("D1: storage failure");
            None
        };
        Some((patch, (state, member, Some(feed))))
    })
    .filter_map(future::ready)
}

/// The worker's time, once a second.
fn clock(_: AppState, _: auth::Member, (): ()) -> impl Stream<Item = Patches<Counter>> {
    stream::unfold((), |()| async {
        // `Delay` waits on a JavaScript timer, which is not `Send`. A worker
        // runs on one thread, so the wrapper never moves it to another.
        SendFuture::new(Delay::from(Duration::from_secs(1))).await;
        Some((Counter::now().patch(Timestamp::now()).into(), ()))
    })
}

/// The name of the session cookie. It holds a random session id and nothing else.
const SESSION_COOKIE: &str = "__Host-session";

/// The Google OAuth client, from `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`
/// and `GOOGLE_REDIRECT_URL`. Any missing or unacceptable value gives `None`.
fn google_config(env: &Env) -> Option<oidc::Config> {
    let var = |name: &str| env.var(name).ok().map(|value| value.to_string());
    oidc::Config::new(
        &var("GOOGLE_CLIENT_ID")?,
        &env.secret("GOOGLE_CLIENT_SECRET").ok()?.to_string(),
        &var("GOOGLE_REDIRECT_URL")?,
    )
    .ok()
}

#[event(fetch)]
async fn fetch(
    request: HttpRequest,
    env: Env,
    _context: Context,
) -> worker::Result<axum::http::Response<axum::body::Body>> {
    let Some(oidc) = google_config(&env) else {
        return Ok(StatusCode::SERVICE_UNAVAILABLE.into_response());
    };
    let state = AppState {
        oidc,
        db: SendWrapper::new(Rc::new(env.d1("DB")?)),
        hub: Arc::new(env.durable_object("HUB")?.get_by_name(HUB)?),
        vapid_public: env
            .var("VAPID_PUBLIC_KEY")
            .map(|v| v.to_string())
            .unwrap_or_default(),
    };
    let sessions = SessionManagerLayer::new(session_store::D1SessionStore::new(state.db.clone()))
        .with_name(SESSION_COOKIE)
        .with_secure(true)
        .with_http_only(true)
        .with_same_site(SameSite::Lax)
        .with_path("/")
        .with_expiry(Expiry::OnInactivity(time::Duration::minutes(15)));
    let mut app = auth::router(assets::routes(pages(Router::new())))
        .layer(axum::middleware::map_response(assets::add_head_links))
        .hypersig_runtime()
        .action(notifications::REGISTER, notifications::register)
        .action(notifications::UNREGISTER, notifications::unregister)
        .action(
            notifications::SUBSCRIPTION_STATUS,
            notifications::subscription_status,
        )
        .action(notifications::SCHEDULE_PROBE, notifications::schedule)
        .action(DOUBLE, double)
        .action(SAVE, save)
        .action(medications::SAVE, medications::save)
        .action(medication_list::REMOVE, medication_list::remove)
        .action(medications::LOAD, medications::load)
        .action(
            medication_edit::SAVE_MEDICATION,
            medication_edit::save_medication,
        )
        .action(medication_edit::SAVE_SLOT, medication_edit::save_slot)
        .action(medication_edit::REMOVE_SLOT, medication_edit::remove_slot)
        .action(pause_edit::SAVE_PAUSE, pause_edit::save_pause)
        .action(pause_edit::REMOVE_PAUSE, pause_edit::remove_pause)
        .stream(CLOCK, clock)
        .stream(SAVES, saves)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            auth::protect,
        ))
        .layer(sessions)
        .with_state(state);
    Ok(app.call(request).await?)
}

/// Runs every minute, from the cron trigger in `cloudflare.config.ts`.
#[event(scheduled)]
async fn scheduled(_event: worker::ScheduledEvent, env: Env, _context: worker::ScheduleContext) {
    if let Ok(db) = env.d1("DB")
        && auth_store::purge_expired(&SendWrapper::new(Rc::new(db)), auth_store::now_ms())
            .await
            .is_err()
    {
        console_error!("scheduled: expired record cleanup failure");
    }
    if notifications::run_scheduled(&env).await.is_err() {
        console_error!("scheduled: notification failure");
    }
}
