//! Token and cost budgets.
//!
//! A call reserves its estimated cost against every applicable account
//! (job, day, and later repo and team) before it runs, then commits the
//! actual usage. If any account would go over its limit, nothing is
//! reserved and the call does not run.
//!
//! Accounts are in memory for now; Phase 2 persists them in the database.

use std::sync::{Arc, Mutex, MutexGuard};

use fixloop_core::config::BudgetLimit;

use crate::error::LlmError;

#[derive(Debug, Default, Clone, Copy)]
struct Ledger {
    tokens: u64,
    cost: f64,
    reserved_tokens: u64,
    reserved_cost: f64,
}

/// One budget scope, such as a job or a day.
#[derive(Debug)]
pub struct BudgetAccount {
    name: String,
    limit: BudgetLimit,
    ledger: Mutex<Ledger>,
}

impl BudgetAccount {
    /// A new, empty account.
    pub fn new(name: impl Into<String>, limit: BudgetLimit) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            limit,
            ledger: Mutex::new(Ledger::default()),
        })
    }

    /// Account name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Tokens and dollars actually spent (excluding open reservations).
    pub fn spent(&self) -> (u64, f64) {
        let l = self.lock();
        (l.tokens, l.cost)
    }

    fn lock(&self) -> MutexGuard<'_, Ledger> {
        // A poisoned lock only means another thread panicked mid-update of
        // plain numbers; the data is still usable.
        self.ledger.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn check(&self, l: &Ledger, tokens: u64, cost: f64) -> Result<(), LlmError> {
        if let Some(max) = self.limit.max_tokens {
            let would = l.tokens + l.reserved_tokens + tokens;
            if would > max {
                return Err(LlmError::BudgetExceeded {
                    scope: self.name.clone(),
                    detail: format!(
                        "{would} tokens needed, limit {max} ({} already used)",
                        l.tokens
                    ),
                });
            }
        }
        if let Some(max) = self.limit.max_cost_usd {
            let would = l.cost + l.reserved_cost + cost;
            if would > max {
                return Err(LlmError::BudgetExceeded {
                    scope: self.name.clone(),
                    detail: format!(
                        "${would:.4} needed, limit ${max:.2} (${:.4} already used)",
                        l.cost
                    ),
                });
            }
        }
        Ok(())
    }
}

/// Reserved budget for one call. Dropping it without [`commit`](Self::commit)
/// releases the reservation.
#[derive(Debug)]
pub struct Reservation {
    accounts: Vec<Arc<BudgetAccount>>,
    tokens: u64,
    cost: f64,
}

/// Reserve `tokens` and `cost` against every account, or none.
pub fn reserve(
    accounts: &[Arc<BudgetAccount>],
    tokens: u64,
    cost: f64,
) -> Result<Reservation, LlmError> {
    let mut reserved: Vec<Arc<BudgetAccount>> = Vec::with_capacity(accounts.len());
    for acc in accounts {
        let mut l = acc.lock();
        if let Err(e) = acc.check(&l, tokens, cost) {
            drop(l);
            release(&reserved, tokens, cost);
            return Err(e);
        }
        l.reserved_tokens += tokens;
        l.reserved_cost += cost;
        reserved.push(Arc::clone(acc));
    }
    Ok(Reservation {
        accounts: reserved,
        tokens,
        cost,
    })
}

fn release(accounts: &[Arc<BudgetAccount>], tokens: u64, cost: f64) {
    for acc in accounts {
        let mut l = acc.lock();
        l.reserved_tokens = l.reserved_tokens.saturating_sub(tokens);
        l.reserved_cost = (l.reserved_cost - cost).max(0.0);
    }
}

impl Reservation {
    /// Replace the reservation with actual usage.
    pub fn commit(mut self, tokens: u64, cost: f64) {
        let accounts = std::mem::take(&mut self.accounts);
        release(&accounts, self.tokens, self.cost);
        for acc in &accounts {
            let mut l = acc.lock();
            l.tokens += tokens;
            l.cost += cost;
        }
    }
}

impl Drop for Reservation {
    fn drop(&mut self) {
        release(&self.accounts, self.tokens, self.cost);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limit(tokens: Option<u64>, cost: Option<f64>) -> BudgetLimit {
        BudgetLimit {
            max_tokens: tokens,
            max_cost_usd: cost,
        }
    }

    #[test]
    fn commit_records_actual_usage() {
        let a = BudgetAccount::new("job", limit(Some(1000), None));
        reserve(std::slice::from_ref(&a), 800, 0.0)
            .unwrap()
            .commit(300, 0.01);
        assert_eq!(a.spent(), (300, 0.01));
        // 300 used + 800 reserved > 1000
        assert!(reserve(std::slice::from_ref(&a), 800, 0.0).is_err());
        assert!(reserve(std::slice::from_ref(&a), 700, 0.0).is_ok());
    }

    #[test]
    fn dropped_reservation_is_released() {
        let a = BudgetAccount::new("job", limit(Some(100), None));
        let r = reserve(std::slice::from_ref(&a), 100, 0.0).unwrap();
        assert!(reserve(std::slice::from_ref(&a), 1, 0.0).is_err());
        drop(r);
        assert!(reserve(std::slice::from_ref(&a), 100, 0.0).is_ok());
    }

    #[test]
    fn all_or_nothing_across_accounts() {
        let job = BudgetAccount::new("job", limit(None, None));
        let day = BudgetAccount::new("daily", limit(None, Some(1.0)));
        let e = reserve(&[job.clone(), day.clone()], 10, 2.0).unwrap_err();
        assert!(e.to_string().contains("daily"), "{e}");
        // The job account must not keep a reservation from the failed attempt.
        let job_tight = BudgetAccount::new("job", limit(Some(10), None));
        assert!(reserve(&[job_tight.clone(), day.clone()], 10, 2.0).is_err());
        assert!(reserve(&[job_tight], 10, 0.0).is_ok());
    }
}
