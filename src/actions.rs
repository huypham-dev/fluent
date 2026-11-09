use crate::settings::Model;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionId {
    Draft,
    Explain,
    Translate,
}

pub struct ActionInfo {
    pub title: &'static str,
    pub editable: bool,
    pub input_label: &'static str,
}

impl ActionId {
    pub fn info(self) -> ActionInfo {
        match self {
            ActionId::Draft => {
                ActionInfo {
                    title: "Draft reply", editable: true, input_label: "Rewrite: shorter, decline politely…"
                }
            }
            ActionId::Explain => {
                ActionInfo { title: "Explain", editable: false, input_label: "Ask a follow-up question…" }
            }
            ActionId::Translate => {
                ActionInfo { title: "Translate", editable: true, input_label: "Rewrite: more casual, shorter…" }
            }
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            ActionId::Draft => "draft",
            ActionId::Explain => "explain",
            ActionId::Translate => "translate",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        [ActionId::Draft, ActionId::Explain, ActionId::Translate].into_iter().find(|a| a.key() == key)
    }

    /// Explain favors speed. Translate stays on the chosen model because Haiku often picks
    /// the wrong target language when it must choose between the primary and secondary one.
    pub fn model(self, chosen: Model) -> Model {
        if self == ActionId::Explain { Model::Haiku } else { chosen }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explain_is_read_only_and_the_others_editable() {
        assert!(!ActionId::Explain.info().editable);
        assert!(ActionId::Draft.info().editable);
        assert!(ActionId::Translate.info().editable);
    }

    #[test]
    fn uses_the_chosen_model_for_draft_and_translate() {
        for action in [ActionId::Draft, ActionId::Translate] {
            assert_eq!(action.model(Model::Sonnet), Model::Sonnet);
            assert_eq!(action.model(Model::Haiku), Model::Haiku);
        }
    }

    #[test]
    fn always_uses_haiku_for_explain() {
        assert_eq!(ActionId::Explain.model(Model::Sonnet), Model::Haiku);
    }

    #[test]
    fn round_trips_keys() {
        assert_eq!(ActionId::from_key("translate"), Some(ActionId::Translate));
        assert_eq!(ActionId::from_key("other"), None);
    }
}
