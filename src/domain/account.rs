use crate::domain::euro_cent::EuroCent;
use eventsourced::{EventSourced, EvtExt, IntoTaggedEvt};
use serde::{Deserialize, Serialize};
use std::num::NonZeroU64;
use thiserror::Error;
use tracing::{debug, error};
use uuid::Uuid;

pub const ACCOUNT_LIFECYCLE_TAG: &str = "account-lifecycle";

/// An account. Defaults to a zero balance and no snapshot.
#[derive(Debug, Default, Clone)]
pub struct Account {
    snapshot_after: Option<NonZeroU64>,
    state: State,
    evt_count: u64,
}

impl Account {
    #[allow(missing_docs)]
    pub fn with_snapshot_after(self, snapshot_after: Option<NonZeroU64>) -> Self {
        Self {
            snapshot_after,
            ..self
        }
    }
}

/// Commands for an eventsourced [Account].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cmd {
    Create(Uuid),
    Deposit(Uuid, EuroCent),
    Withdraw(Uuid, EuroCent),
}

/// Events for an eventsourced [Account].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Evt {
    Created(Uuid),
    Deposited {
        id: Uuid,
        old_balance: EuroCent,
        amount: EuroCent,
    },
    Withdrawn {
        id: Uuid,
        old_balance: EuroCent,
        amount: EuroCent,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    #[default]
    NonExistent,
    Created {
        id: Uuid,
        balance: EuroCent,
    },
}

/// Command handler errors for an eventsourced [Account].
#[derive(Debug, Clone, Error)]
pub enum Error {
    #[error("Balance '{balance}' insufficient to withwraw amount '{withdraw_amount}'")]
    InvalidWithdraw {
        balance: EuroCent,
        withdraw_amount: EuroCent,
    },

    #[error("This account has not been created yet")]
    NotYetCreated,

    #[error("This account has already been created")]
    AlreadyCreated,
}

impl EventSourced for Account {
    type Cmd = Cmd;

    type Evt = Evt;

    type State = State;

    type Error = Error;

    fn handle_cmd(&self, cmd: Self::Cmd) -> Result<impl IntoTaggedEvt<Self::Evt>, Self::Error> {
        debug!(?cmd, "Handling command");

        match (self.state, cmd) {
            // In State::NonExistent:
            (State::NonExistent, Cmd::Create(id)) => {
                Ok(Evt::Created(id).with_tag(ACCOUNT_LIFECYCLE_TAG))
            }
            (State::NonExistent, other) => {
                error!("Cannot handle command '{other:?}' in state NonExistent");
                Err(Error::NotYetCreated)
            }

            // In State::Created:
            (State::Created { balance, .. }, Cmd::Deposit(id, amount)) => Ok(Evt::Deposited {
                id,
                old_balance: balance,
                amount,
            }
            .into_tagged_evt()),
            (State::Created { balance, .. }, Cmd::Withdraw(_, amount)) if balance < amount => {
                Err(Error::InvalidWithdraw {
                    balance,
                    withdraw_amount: amount,
                })
            }
            (State::Created { balance, .. }, Cmd::Withdraw(id, amount)) => Ok(Evt::Withdrawn {
                id,
                old_balance: balance,
                amount,
            }
            .into_tagged_evt()),
            (State::Created { .. }, other) => {
                error!("Cannot handle command '{other:?}' in state Created");
                Err(Error::AlreadyCreated)
            }
        }
    }

    fn handle_evt(&mut self, evt: Self::Evt) -> Option<Self::State> {
        debug!(?evt, "Handling event");

        match (self.state, evt) {
            // In State::NonExistent:
            (State::NonExistent, Evt::Created(id)) => self.set_state(State::Created {
                id,
                balance: EuroCent::default(),
            }),

            (State::NonExistent, _) => panic!("Illegal event '{evt:?}' in state NonExistent"),

            // In State::Created:
            (
                State::Created { id, balance },
                Evt::Deposited {
                    id: _,
                    old_balance: _,
                    amount,
                },
            ) => self.set_state(State::Created {
                id,
                balance: balance + amount,
            }),

            (
                State::Created { id, balance },
                Evt::Withdrawn {
                    id: _,
                    old_balance: _,
                    amount,
                },
            ) => self.set_state(State::Created {
                id,
                balance: balance - amount,
            }),

            (State::Created { .. }, _) => panic!("Illegal event '{evt:?}' in state Created"),
        }

        self.evt_count += 1;
        self.snapshot_after
            .filter(|snapshot_after| self.evt_count % snapshot_after.get() == 0)
            .map(|_| {
                debug!(self.evt_count, "Taking snapshot");
                self.state
            })
    }

    fn set_state(&mut self, state: Self::State) {
        self.state = state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_cmd_and_evt() {
        let mut account = Account::default();

        // Command Deposit fails in state NotCreated.
        assert!(account
            .handle_cmd(Cmd::Deposit(Uuid::now_v7(), 1u64.into()))
            .is_err());

        // Command Withdraw fails in state NotCreated.
        assert!(account
            .handle_cmd(Cmd::Withdraw(Uuid::now_v7(), 1u64.into()))
            .is_err());

        // Command Create succeeds in state NotCreated.
        assert!(account.handle_cmd(Cmd::Create(Uuid::now_v7())).is_ok());

        // Handle event Created.
        account.handle_evt(Evt::Created(Uuid::now_v7()));

        // Command Withdraw fails in state Created with insufficient balance.
        assert!(account
            .handle_cmd(Cmd::Withdraw(Uuid::now_v7(), 1u64.into()))
            .is_err());

        // Handle event Deposited.
        account.handle_evt(Evt::Deposited {
            id: Uuid::now_v7(),
            old_balance: 0u64.into(),
            amount: 1u64.into(),
        });

        // Command Withdraw succeeds in state Created.
        assert!(account
            .handle_cmd(Cmd::Withdraw(Uuid::now_v7(), 1u64.into()))
            .is_ok());

        // Handle event Withdrawn.
        account.handle_evt(Evt::Withdrawn {
            id: Uuid::now_v7(),
            old_balance: 1u64.into(),
            amount: 1u64.into(),
        });

        // Command Withdraw fails in state Created with insufficient balance.
        assert!(account
            .handle_cmd(Cmd::Withdraw(Uuid::now_v7(), 1u64.into()))
            .is_err());
    }
}
