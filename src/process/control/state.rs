// SPDX-FileCopyrightText: 2026 roolrz
// SPDX-License-Identifier: Apache-2.0

//! Bounded disposition and consumption state, independent of transport.

use super::{Dispatch, Disposition};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Opening,
    Active(Disposition),
    Legacy,
    Lost,
}

pub(super) struct State {
    phase: Phase,
    pub(super) generation: u64,
    pending: bool,
    pub(super) in_flight: Option<u64>,
    next_event: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Action {
    None,
    Dispatch(Dispatch),
    Notify { generation: u64, id: u64 },
}

impl State {
    pub(super) const fn new() -> Self {
        Self {
            phase: Phase::Opening,
            generation: 0,
            pending: false,
            in_flight: None,
            next_event: 1,
        }
    }

    pub(super) fn request(&mut self) {
        self.pending = true;
    }

    pub(super) fn configure(&mut self, generation: u64, disposition: Disposition) -> bool {
        if !matches!(self.phase, Phase::Opening | Phase::Active(_))
            || self.generation.checked_add(1) != Some(generation)
        {
            return false;
        }
        self.generation = generation;
        self.phase = Phase::Active(disposition);
        true
    }

    pub(super) fn disconnected(&mut self) {
        self.phase = if self.phase == Phase::Opening {
            Phase::Legacy
        } else {
            Phase::Lost
        };
        self.in_flight = None;
    }

    pub(super) fn failed(&mut self) {
        self.phase = Phase::Lost;
        self.in_flight = None;
    }

    pub(super) fn consumed(&mut self, id: u64) -> bool {
        if self.in_flight != Some(id) {
            return false;
        }
        self.in_flight = None;
        true
    }

    pub(super) fn action(&mut self) -> Option<Action> {
        if !self.pending {
            return Some(Action::None);
        }
        let outcome = match self.phase {
            Phase::Active(Disposition::Notify) => {
                if self.in_flight.is_some() {
                    return Some(Action::None);
                }
                let id = self.next_event;
                self.next_event = id.checked_add(1)?;
                self.pending = false;
                self.in_flight = Some(id);
                return Some(Action::Notify {
                    generation: self.generation,
                    id,
                });
            }
            Phase::Opening | Phase::Legacy | Phase::Active(Disposition::Default) => {
                Dispatch::Default
            }
            Phase::Active(Disposition::Ignore) => Dispatch::Ignore,
            Phase::Lost => Dispatch::Unavailable,
        };
        self.pending = false;
        Some(Action::Dispatch(outcome))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_unregistered_disconnect_selects_legacy_default() {
        let mut legacy = State::new();
        legacy.disconnected();
        legacy.request();
        assert_eq!(legacy.action(), Some(Action::Dispatch(Dispatch::Default)));
        let mut registered = State::new();
        assert!(registered.configure(1, Disposition::Notify));
        registered.disconnected();
        registered.request();
        assert_eq!(
            registered.action(),
            Some(Action::Dispatch(Dispatch::Unavailable))
        );
    }

    #[test]
    fn unregistered_programs_have_explicit_default_disposition() {
        let mut state = State::new();
        state.request();
        assert_eq!(state.action(), Some(Action::Dispatch(Dispatch::Default)));
        assert!(state.configure(1, Disposition::Notify));
        state.request();
        assert_eq!(
            state.action(),
            Some(Action::Notify {
                generation: 1,
                id: 1
            })
        );
    }

    #[test]
    fn slow_consumer_has_one_notification_and_one_coalesced_request() {
        let mut state = State::new();
        assert!(state.configure(1, Disposition::Notify));
        state.request();
        assert_eq!(
            state.action(),
            Some(Action::Notify {
                generation: 1,
                id: 1
            })
        );
        for _ in 0..1000 {
            state.request();
            assert_eq!(state.action(), Some(Action::None));
        }
        assert!(!state.consumed(2));
        assert!(state.consumed(1));
        assert_eq!(
            state.action(),
            Some(Action::Notify {
                generation: 1,
                id: 2
            })
        );
    }

    #[test]
    fn configuration_changes_do_not_revoke_an_existing_delivery() {
        let mut state = State::new();
        assert!(state.configure(1, Disposition::Notify));
        state.request();
        assert_eq!(
            state.action(),
            Some(Action::Notify {
                generation: 1,
                id: 1
            })
        );
        assert!(state.configure(2, Disposition::Ignore));
        assert_eq!(state.in_flight, Some(1));
        state.request();
        assert_eq!(state.action(), Some(Action::Dispatch(Dispatch::Ignore)));
        assert!(state.consumed(1));
        assert!(!state.configure(2, Disposition::Default));
        assert!(!state.configure(4, Disposition::Default));
    }

    #[test]
    fn malformed_unregistered_client_is_not_a_legacy_client() {
        let mut state = State::new();
        state.failed();
        state.request();
        assert_eq!(
            state.action(),
            Some(Action::Dispatch(Dispatch::Unavailable))
        );
        assert!(!state.configure(1, Disposition::Notify));
    }

    #[test]
    fn default_and_ignore_dispatch_once_without_waiting_for_the_application() {
        for (disposition, expected) in [
            (Disposition::Default, Dispatch::Default),
            (Disposition::Ignore, Dispatch::Ignore),
        ] {
            let mut state = State::new();
            assert!(state.configure(1, disposition));
            for _ in 0..1000 {
                state.request();
            }
            assert_eq!(state.action(), Some(Action::Dispatch(expected)));
            assert_eq!(state.action(), Some(Action::None));
            assert_eq!(state.in_flight, None);
        }
    }

    #[test]
    fn registration_and_notification_generations_cannot_wrap() {
        let mut state = State::new();
        state.generation = u64::MAX;
        assert!(!state.configure(0, Disposition::Default));
        let mut state = State::new();
        assert!(state.configure(1, Disposition::Notify));
        state.next_event = u64::MAX;
        state.request();
        assert_eq!(state.action(), None);
        assert_eq!(state.in_flight, None);
    }
}
