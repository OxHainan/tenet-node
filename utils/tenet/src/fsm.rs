use serde::{Serialize, Deserialize};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum State {
    Default,
    Challenging,
    Frozen,
    Responsed,
    Timeout,
    Punished,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Event {
    Challenge,
    Response,
    Freeze,
    Unfreeze,
}

#[derive(Debug)]
struct TransitionError;

struct Fsm {
    state: State,
}
impl Fsm {
    fn new() -> Self {
        Fsm { state: State::Default }
    }

    fn transition(&mut self, event: Event) -> Result<(), TransitionError> {
        let event_clone = event.clone();
        match (&self.state, event) {
            (&State::Default, Event::Challenge) => {
                self.state = State::Challenging;
                Ok(())
            }
            (&State::Challenging, Event::Response) => {
                self.state = State::Responsed;
                Ok(())
            }
            (&State::Challenging, Event::Freeze) => {
                self.state = State::Frozen;
                Ok(())
            }
            (&State::Frozen, Event::Unfreeze) => {
                self.state = State::Challenging;
                Ok(())
            }
            _ => {
                println!("No transition for {:?} in state {:?}", event_clone, self.state);
                Err(TransitionError)
            }
        }
    }

    fn current_state(&self) -> &State {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fsm_transition() {
        let mut fsm = Fsm::new();

        assert!(fsm.transition(Event::Challenge).is_ok());
        assert_eq!(*fsm.current_state(), State::Challenging);

        assert!(fsm.transition(Event::Freeze).is_ok());
        assert_eq!(*fsm.current_state(), State::Frozen);

        assert!(fsm.transition(Event::Unfreeze).is_ok());
        assert_eq!(*fsm.current_state(), State::Challenging);

        assert!(fsm.transition(Event::Response).is_ok());
        assert_eq!(*fsm.current_state(), State::Responsed);
    
    }
}