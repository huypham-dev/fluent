use crate::actions::ActionId;
use crate::cli::CliError;
use crate::prompt::{FollowUp, Languages, system_prompt, user_prompt};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Generating,
    Ready,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SessionState {
    pub phase: Phase,
    pub result: String,
    pub error: Option<CliError>,
}

#[derive(Debug, PartialEq)]
pub struct GenerateRequest {
    pub system: String,
    pub prompt: String,
}

/// A generation the caller must start. Its events carry `id` back to the session.
#[derive(Debug)]
pub struct Run {
    pub id: u64,
    pub request: GenerateRequest,
}

pub struct SessionConfig {
    pub action: ActionId,
    pub selected_text: String,
    pub primary_language: String,
    pub secondary_language: String,
    /// Read for each generation, so edits to the style apply at once.
    pub load_style: Box<dyn Fn() -> Option<String>>,
}

/// The state of one action, from the first generation through any follow-ups.
/// It decides what to generate; the caller runs it and reports back.
pub struct ActionSession {
    config: SessionConfig,
    pub state: SessionState,
    run_id: u64,
    last_prompt: String,
    previous_result: String,
    received_text: bool,
}

impl ActionSession {
    pub fn new(config: SessionConfig) -> Self {
        let last_prompt = user_prompt(&config.selected_text, None);
        Self {
            config,
            state: SessionState { phase: Phase::Generating, result: String::new(), error: None },
            run_id: 0,
            last_prompt,
            previous_result: String::new(),
            received_text: false,
        }
    }

    pub fn action(&self) -> ActionId {
        self.config.action
    }

    pub fn start(&mut self) -> Run {
        self.run(self.last_prompt.clone())
    }

    pub fn retry(&mut self) -> Run {
        self.run(self.last_prompt.clone())
    }

    pub fn submit_input(&mut self, text: &str) -> Option<Run> {
        let instruction = text.trim();
        if instruction.is_empty() {
            return None;
        }
        let prompt =
            user_prompt(&self.config.selected_text, Some(FollowUp { current_result: &self.state.result, instruction }));
        Some(self.run(prompt))
    }

    pub fn set_result(&mut self, text: &str) {
        if self.state.phase != Phase::Generating {
            self.state.result = text.to_string();
        }
    }

    /// Makes events from the running generation stale. The caller stops the process.
    pub fn cancel(&mut self) {
        self.run_id += 1;
    }

    /// Returns whether the state changed.
    pub fn on_text(&mut self, id: u64, text: &str) -> bool {
        if id != self.run_id {
            return false;
        }
        self.received_text = true;
        self.state.result.push_str(text);
        true
    }

    /// Returns whether the state changed.
    pub fn on_done(&mut self, id: u64, result: Result<(), CliError>) -> bool {
        if id != self.run_id {
            return false;
        }
        match result {
            Ok(()) => self.state.phase = Phase::Ready,
            Err(error) => {
                if !self.received_text {
                    self.state.result = self.previous_result.clone();
                }
                self.state.phase = Phase::Failed;
                self.state.error = Some(error);
            }
        }
        true
    }

    fn run(&mut self, prompt: String) -> Run {
        self.cancel();
        self.previous_result = std::mem::take(&mut self.state.result);
        self.received_text = false;
        self.last_prompt = prompt.clone();
        self.state = SessionState { phase: Phase::Generating, result: String::new(), error: None };
        let style = (self.config.load_style)();
        let languages =
            Languages { primary: &self.config.primary_language, secondary: &self.config.secondary_language };
        Run {
            id: self.run_id,
            request: GenerateRequest {
                system: system_prompt(self.config.action, &languages, style.as_deref()),
                prompt,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::CliErrorKind;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn session_with_style(style: Rc<RefCell<Option<String>>>) -> ActionSession {
        ActionSession::new(SessionConfig {
            action: ActionId::Draft,
            selected_text: "Review my PR?".into(),
            primary_language: "Vietnamese".into(),
            secondary_language: "English".into(),
            load_style: Box::new(move || style.borrow().clone()),
        })
    }

    fn session() -> ActionSession {
        session_with_style(Rc::default())
    }

    fn error(kind: CliErrorKind, message: &str) -> CliError {
        CliError { kind, message: message.into() }
    }

    fn finish(session: &mut ActionSession, run: &Run, pieces: &[&str], result: Result<(), CliError>) {
        for piece in pieces {
            session.on_text(run.id, piece);
        }
        session.on_done(run.id, result);
    }

    #[test]
    fn accumulates_text_and_becomes_ready() {
        let mut s = session();
        let run = s.start();
        assert_eq!(s.state.phase, Phase::Generating);
        finish(&mut s, &run, &["Sure", ", today"], Ok(()));
        assert_eq!(s.state.phase, Phase::Ready);
        assert_eq!(s.state.result, "Sure, today");
    }

    #[test]
    fn keeps_partial_text_on_failure() {
        let mut s = session();
        let run = s.start();
        finish(&mut s, &run, &["Sure"], Err(error(CliErrorKind::Failed, "usage limit")));
        assert_eq!(s.state.phase, Phase::Failed);
        assert_eq!(s.state.result, "Sure");
        assert_eq!(s.state.error, Some(error(CliErrorKind::Failed, "usage limit")));
    }

    #[test]
    fn sends_the_edited_result_in_a_follow_up() {
        let mut s = session();
        let run = s.start();
        finish(&mut s, &run, &["First"], Ok(()));
        s.set_result("My edit");
        let follow = s.submit_input("shorter").unwrap();
        assert!(follow.request.prompt.contains("My edit"));
        assert!(follow.request.prompt.contains("shorter"));
        assert!(follow.request.prompt.contains("Review my PR?"));
        finish(&mut s, &follow, &["Short"], Ok(()));
        assert_eq!(s.state.result, "Short");
    }

    #[test]
    fn restores_the_edit_when_a_follow_up_fails_with_no_output() {
        let mut s = session();
        let run = s.start();
        finish(&mut s, &run, &["First"], Ok(()));
        s.set_result("My edit");
        let follow = s.submit_input("shorter").unwrap();
        finish(&mut s, &follow, &[], Err(error(CliErrorKind::Timeout, "slow")));
        assert_eq!(s.state.phase, Phase::Failed);
        assert_eq!(s.state.result, "My edit");
    }

    #[test]
    fn retries_the_last_request() {
        let mut s = session();
        let run = s.start();
        finish(&mut s, &run, &["First"], Ok(()));
        s.set_result("My edit");
        let follow = s.submit_input("shorter").unwrap();
        finish(&mut s, &follow, &[], Err(error(CliErrorKind::Timeout, "slow")));
        let retry = s.retry();
        assert_eq!(retry.request.prompt, follow.request.prompt);
        finish(&mut s, &retry, &["Short"], Ok(()));
        assert_eq!(s.state.phase, Phase::Ready);
        assert_eq!(s.state.result, "Short");
    }

    #[test]
    fn ignores_events_from_a_replaced_or_cancelled_run() {
        let mut s = session();
        let old = s.start();
        s.on_text(old.id, "Old");
        let new = s.submit_input("shorter").unwrap();
        assert!(!s.on_text(old.id, "late"));
        assert!(!s.on_done(old.id, Ok(())));
        finish(&mut s, &new, &["New"], Ok(()));
        assert_eq!(s.state.result, "New");
        s.cancel();
        assert!(!s.on_text(new.id, "x"));
    }

    #[test]
    fn ignores_blank_input() {
        let mut s = session();
        s.start();
        assert!(s.submit_input("   ").is_none());
    }

    #[test]
    fn ignores_edits_while_generating() {
        let mut s = session();
        let run = s.start();
        s.on_text(run.id, "x");
        s.set_result("mine");
        assert_eq!(s.state.result, "x");
    }

    #[test]
    fn reads_the_style_for_each_generation() {
        let style = Rc::new(RefCell::new(Some("first style".to_string())));
        let mut s = session_with_style(style.clone());
        let first = s.start();
        finish(&mut s, &first, &[], Ok(()));
        *style.borrow_mut() = Some("second style".into());
        let second = s.submit_input("again").unwrap();
        assert!(first.request.system.contains("first style"));
        assert!(second.request.system.contains("second style"));
    }
}
