use serde::{Deserialize, Serialize};

use crate::types::PlayerId;

/// The current phase of the turn cycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnPhase {
    WaitingForPlayer(PlayerId),
    BetweenTurns,
}

/// Result of advancing the turn state machine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnTransition {
    NextPlayer(PlayerId),
    NewTurn(u32),
}

/// Manages the turn order for all players.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnStateMachine {
    pub phase: TurnPhase,
    player_order: Vec<PlayerId>,
    current_index: usize,
    pub turn_number: u32,
}

impl TurnStateMachine {
    /// Creates a new turn state machine starting at turn 1 with the first player.
    pub fn new(player_order: Vec<PlayerId>) -> Self {
        let first = player_order[0];
        TurnStateMachine {
            phase: TurnPhase::WaitingForPlayer(first),
            player_order,
            current_index: 0,
            turn_number: 1,
        }
    }

    /// Returns the player whose turn it currently is.
    pub fn current_player(&self) -> PlayerId {
        self.player_order[self.current_index]
    }

    /// Returns true if it is the given player's turn.
    pub fn is_player_turn(&self, player: PlayerId) -> bool {
        self.phase == TurnPhase::WaitingForPlayer(player)
    }

    /// Returns the number of players in the turn order.
    pub fn player_count(&self) -> usize {
        self.player_order.len()
    }

    /// Advances to the next player, or wraps to a new turn if all players
    /// have taken their turn. Returns the transition that occurred.
    pub fn advance_player(&mut self) -> TurnTransition {
        self.current_index += 1;
        if self.current_index >= self.player_order.len() {
            self.current_index = 0;
            self.turn_number += 1;
            let player = self.player_order[self.current_index];
            self.phase = TurnPhase::WaitingForPlayer(player);
            TurnTransition::NewTurn(self.turn_number)
        } else {
            let player = self.player_order[self.current_index];
            self.phase = TurnPhase::WaitingForPlayer(player);
            TurnTransition::NextPlayer(player)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_three_player_cycle() {
        let mut tsm = TurnStateMachine::new(vec![PlayerId(0), PlayerId(1), PlayerId(2)]);
        assert_eq!(tsm.current_player(), PlayerId(0));
        assert_eq!(tsm.turn_number, 1);

        // Player 0 -> Player 1
        let t = tsm.advance_player();
        assert_eq!(t, TurnTransition::NextPlayer(PlayerId(1)));
        assert_eq!(tsm.current_player(), PlayerId(1));

        // Player 1 -> Player 2
        let t = tsm.advance_player();
        assert_eq!(t, TurnTransition::NextPlayer(PlayerId(2)));
        assert_eq!(tsm.current_player(), PlayerId(2));

        // Player 2 -> Player 0, new turn
        let t = tsm.advance_player();
        assert_eq!(t, TurnTransition::NewTurn(2));
        assert_eq!(tsm.current_player(), PlayerId(0));
        assert_eq!(tsm.turn_number, 2);
    }

    #[test]
    fn test_is_player_turn() {
        let tsm = TurnStateMachine::new(vec![PlayerId(0), PlayerId(1)]);
        assert!(tsm.is_player_turn(PlayerId(0)));
        assert!(!tsm.is_player_turn(PlayerId(1)));
    }

    #[test]
    fn test_two_player_two_turns() {
        let mut tsm = TurnStateMachine::new(vec![PlayerId(0), PlayerId(1)]);

        // Turn 1: P0 -> P1
        assert_eq!(
            tsm.advance_player(),
            TurnTransition::NextPlayer(PlayerId(1))
        );
        // Turn 1: P1 -> P0, new turn 2
        assert_eq!(tsm.advance_player(), TurnTransition::NewTurn(2));
        assert_eq!(tsm.current_player(), PlayerId(0));

        // Turn 2: P0 -> P1
        assert_eq!(
            tsm.advance_player(),
            TurnTransition::NextPlayer(PlayerId(1))
        );
        // Turn 2: P1 -> P0, new turn 3
        assert_eq!(tsm.advance_player(), TurnTransition::NewTurn(3));
        assert_eq!(tsm.turn_number, 3);
    }

    #[test]
    fn test_single_player() {
        let mut tsm = TurnStateMachine::new(vec![PlayerId(0)]);
        assert_eq!(tsm.current_player(), PlayerId(0));

        let t = tsm.advance_player();
        assert_eq!(t, TurnTransition::NewTurn(2));
        assert_eq!(tsm.current_player(), PlayerId(0));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let tsm = TurnStateMachine::new(vec![PlayerId(0), PlayerId(1), PlayerId(2)]);
        let json = serde_json::to_string(&tsm).unwrap();
        let back: TurnStateMachine = serde_json::from_str(&json).unwrap();
        assert_eq!(back.current_player(), PlayerId(0));
        assert_eq!(back.turn_number, 1);
    }
}
