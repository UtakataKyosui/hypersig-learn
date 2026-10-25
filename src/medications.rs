//! Typed aggregate editing contract. Presentation controls belong to MVP 3.
use crate::{
    AppState,
    auth::Member,
    domain::{Medication, UtcInstant},
    medication_input::{DoseSlotInput, MedicationInput, PauseInput},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
};
use hypersig::{Action, Form, Page, Patches, Signals};
use serde::{Deserialize, Serialize};

#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct Editor {
    pub id: String,
    /// Zero creates a new medication; every successful mutation increments it.
    pub revision: i32,
    pub enabled: bool,
    pub medication: MedicationInput,
    pub slots: Vec<Slot>,
    pub pauses: Vec<Pause>,
}
#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct Slot {
    pub id: String,
    pub input: DoseSlotInput,
}
#[derive(Clone, Signals, Serialize, Deserialize)]
pub struct Pause {
    pub id: String,
    pub input: PauseInput,
}
impl hypersig::Validate for Editor {
    fn rules<S>(_: hypersig::Sig<S, Self>, _: &mut hypersig::Rules<S, Self>) {}
}
impl Editor {
    pub fn domain(&self, saved_at: i64) -> Result<Medication, Error> {
        let id = self.id.parse().map_err(|_| Error::Invalid)?;
        let (name, schedule) = self.medication.parse().map_err(|_| Error::Invalid)?;
        if self.revision < 0
            || self.revision == i32::MAX
            || self.slots.len() > 64
            || self.pauses.len() > 128
        {
            return Err(Error::Invalid);
        }
        let mut slots = Vec::new();
        for slot in &self.slots {
            if slots
                .iter()
                .any(|s: &crate::domain::DoseSlot| s.id.to_string() == slot.id)
            {
                return Err(Error::Invalid);
            }
            slots.push(
                slot.input
                    .parse(slot.id.parse().map_err(|_| Error::Invalid)?)
                    .map_err(|_| Error::Invalid)?,
            );
        }
        let mut pauses = Vec::new();
        let mut ids = std::collections::HashSet::new();
        for pause in &self.pauses {
            pause
                .id
                .parse::<crate::domain::MedicationId>()
                .map_err(|_| Error::Invalid)?;
            if !ids.insert(&pause.id) {
                return Err(Error::Invalid);
            }
            pauses.push(pause.input.parse(&pauses).map_err(|_| Error::Invalid)?);
        }
        Ok(Medication {
            id,
            name,
            enabled: self.enabled,
            schedule,
            slots,
            pauses,
            saved_at: UtcInstant(saved_at),
        })
    }
}
#[derive(Signals, Serialize, Deserialize)]
pub struct Summary {
    pub id: String,
    pub name: String,
    pub revision: i32,
    pub next_at: Option<hypersig::Timestamp>,
}
#[derive(Signals, Serialize, Deserialize)]
pub struct Settings {
    pub form: Form<Editor>,
    pub items: Vec<Summary>,
    pub problem: String,
}
#[derive(Serialize, Deserialize)]
pub struct Delete {
    pub id: String,
    pub revision: i32,
}
pub const SAVE: Action<Settings, Editor> = Action::post("/medications/save");
pub const DELETE: Action<Settings, Delete> = Action::post("/medications/delete");
pub const LOAD: Action<Settings, String> = Action::post("/medications/load");
pub enum Error {
    Invalid,
    Missing,
    Conflict,
    Store,
}
impl From<worker::Error> for Error {
    fn from(_: worker::Error) -> Self {
        Self::Store
    }
}
impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Invalid => (StatusCode::BAD_REQUEST, "入力値または休薬期間が不正です。"),
            Self::Missing => (StatusCode::NOT_FOUND, "設定が見つかりません。"),
            Self::Conflict => (
                StatusCode::CONFLICT,
                "別の更新があります。再読み込みしてください。",
            ),
            Self::Store => (StatusCode::INTERNAL_SERVER_ERROR, "保存できませんでした。"),
        };
        (status, message).into_response()
    }
}
pub fn summary(editor: &Editor, saved_at: i64, now: i64) -> Result<Summary, Error> {
    let medication = editor.domain(saved_at)?;
    let next = medication
        .next(UtcInstant(now))
        .map_err(|_| Error::Invalid)?;
    Ok(Summary {
        id: editor.id.clone(),
        name: medication.name,
        revision: editor.revision,
        next_at: match next {
            crate::domain::NextDose::At(o) => Some(hypersig::Timestamp::from_millis(o.at.0)),
            _ => None,
        },
    })
}
fn saved(editor: Editor, now: i64) -> Result<Patches<Settings>, Error> {
    let item = summary(&editor, now, now)?;
    Ok(Patches::new()
        .signals(Settings::form().patch(Form::new(editor)))
        .list(Settings::items().patch_upsert(&[item], |i| i.at(Summary::id()))))
}
pub async fn save(
    state: AppState,
    member: Member,
    mut editor: Editor,
) -> Result<Patches<Settings>, Error> {
    let now = hypersig::Timestamp::now().as_millis();
    if editor.revision > 0 {
        let row = crate::repository::load(&state.db, member.owner(), &editor.id).await?;
        if row.revision != editor.revision {
            return Err(Error::Conflict);
        }
    }
    if editor.domain(now).is_err() {
        return Ok(Settings::form()
            .reject(&hypersig::FieldErrors::new().add(
                Editor::medication(),
                "入力値・重複する休薬期間を確認してください。",
            ))
            .into());
    }
    editor.medication.name = editor.medication.name.trim().into();
    editor.medication.interval = editor.medication.interval.max(1);
    if editor.medication.repeat != "weekdays" {
        editor.medication.weekdays = 0;
    }
    for slot in &mut editor.slots {
        slot.input.unit = slot.input.unit.trim().into();
    }
    crate::repository::save(&state.db, member.owner(), &editor, now).await?;
    editor.revision += 1;
    saved(editor, now)
}
pub async fn delete(
    state: AppState,
    member: Member,
    input: Delete,
) -> Result<Patches<Settings>, Error> {
    crate::repository::delete(&state.db, member.owner(), &input).await?;
    Ok(Settings::items()
        .patch_remove(&[input.id], |i| i.at(Summary::id()))
        .into())
}
pub async fn load(state: AppState, member: Member, id: String) -> Result<Patches<Settings>, Error> {
    let row = crate::repository::load(&state.db, member.owner(), &id).await?;
    Ok(Settings::form().patch(Form::new(row.editor()?)).into())
}
pub async fn page(state: AppState, member: Member, (): ()) -> Result<Page<Settings>, Error> {
    let now = hypersig::Timestamp::now().as_millis();
    let items = crate::repository::list(&state.db, member.owner())
        .await?
        .into_iter()
        .map(|row| summary(&row.editor()?, row.updated_at, now))
        .collect::<Result<Vec<_>, Error>>()?;
    let editor = Editor {
        id: random_id()?,
        revision: 0,
        enabled: true,
        medication: MedicationInput {
            name: String::new(),
            start: UtcInstant(now)
                .tokyo_date()
                .map_err(|_| Error::Invalid)?
                .to_string(),
            end: String::new(),
            repeat: "daily".into(),
            interval: 1,
            weekdays: 0,
        },
        slots: vec![],
        pauses: vec![],
    };
    Ok(Page::new(
        "服薬設定",
        Settings {
            form: Form::new(editor),
            items,
            problem: String::new(),
        },
    )
    .child(hypersig::html::p().text("服薬設定 · Asia/Tokyo"))
    .child(crate::auth::sign_out_button()))
}
pub fn random_id() -> Result<String, Error> {
    use base64::Engine as _;
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Store)?;
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn editor() -> Editor {
        Editor {
            id: "med".into(),
            revision: 1,
            enabled: true,
            medication: MedicationInput {
                name: "薬".into(),
                start: "2026-10-01".into(),
                end: String::new(),
                repeat: "daily".into(),
                interval: 1,
                weekdays: 0,
            },
            slots: vec![Slot {
                id: "slot".into(),
                input: DoseSlotInput {
                    time: "08:00".into(),
                    amount: "0.5".into(),
                    unit: "錠".into(),
                    timing: "after_meal".into(),
                    note: String::new(),
                },
            }],
            pauses: vec![],
        }
    }
    #[test]
    fn saved_form_resets_baseline_and_emits_keyed_upsert() {
        let now = "2026-10-01"
            .parse::<crate::domain::LocalDate>()
            .unwrap()
            .at("00:00".parse().unwrap())
            .0;
        let patches = saved(editor(), now).ok().unwrap();
        let form = patches.value_of(Settings::form()).unwrap();
        assert_eq!(form.value().revision, 1);
        assert_eq!(form.value().medication.name, "薬");
        let event = patches.to_event_stream();
        assert!(event.contains("upsert"));
        let serialized = serde_json::to_value(form).unwrap();
        assert_eq!(serialized["initial"], serialized["value"]);
        assert!(summary(&editor(), now, now).ok().unwrap().next_at.is_some());
    }
    #[test]
    fn duplicate_ids_overlapping_pauses_and_invalid_domain_values_are_refused() {
        let mut e = editor();
        e.slots.push(e.slots[0].clone());
        assert!(e.domain(0).is_err());
        let mut e = editor();
        e.slots[0].input.amount = "NaN".into();
        assert!(e.domain(0).is_err());
        let mut e = editor();
        e.pauses = vec![
            Pause {
                id: "a".into(),
                input: PauseInput {
                    start: "2026-10-03".into(),
                    end: "2026-10-05".into(),
                },
            },
            Pause {
                id: "b".into(),
                input: PauseInput {
                    start: "2026-10-05".into(),
                    end: "2026-10-06".into(),
                },
            },
        ];
        assert!(e.domain(0).is_err());
        e.pauses[1].input.start = "2026-10-06".into();
        assert!(e.domain(0).is_ok());
    }
}
