//! Small concurrency helpers shared by renderer hot-path caches.

use std::hash::Hash;
use std::sync::Arc;

use hashbrown::HashMap;
use parking_lot::{Condvar, Mutex};

/// Per-key single-flight coordinator.
///
/// Callers that acquire a leader permit own the in-flight work for that key. Callers that acquire
/// a waiter permit block until the leader drops its permit, then retry their ordinary cache path.
#[derive(Debug)]
pub(crate) struct KeyedSingleFlight<K>
where
    K: Eq + Hash,
{
    in_flight: Mutex<HashMap<K, Arc<FlightState>>>,
}

impl<K> Default for KeyedSingleFlight<K>
where
    K: Eq + Hash,
{
    fn default() -> Self {
        Self {
            in_flight: Mutex::new(HashMap::new()),
        }
    }
}

impl<K> KeyedSingleFlight<K>
where
    K: Clone + Eq + Hash,
{
    /// Acquires the leader permit for a missing key or joins the existing in-flight work.
    pub(crate) fn acquire(&self, key: K) -> SingleFlightPermit<'_, K> {
        let mut in_flight = self.in_flight.lock();
        if let Some(state) = in_flight.get(&key) {
            return SingleFlightPermit::Waiter(SingleFlightWaiter {
                state: state.clone(),
            });
        }

        let state = Arc::new(FlightState::default());
        in_flight.insert(key.clone(), state.clone());
        drop(in_flight);
        SingleFlightPermit::Leader(SingleFlightLeader {
            owner: self,
            key: Some(key),
            state,
        })
    }
}

/// Result of acquiring a [`KeyedSingleFlight`] key.
#[derive(Debug)]
pub(crate) enum SingleFlightPermit<'a, K>
where
    K: Eq + Hash,
{
    /// The caller should perform the cache-miss work.
    Leader(SingleFlightLeader<'a, K>),
    /// Another caller is already performing the cache-miss work.
    Waiter(SingleFlightWaiter),
}

/// RAII permit for the elected single-flight builder.
#[derive(Debug)]
pub(crate) struct SingleFlightLeader<'a, K>
where
    K: Eq + Hash,
{
    owner: &'a KeyedSingleFlight<K>,
    key: Option<K>,
    state: Arc<FlightState>,
}

impl<K> Drop for SingleFlightLeader<'_, K>
where
    K: Eq + Hash,
{
    fn drop(&mut self) {
        self.complete();
    }
}

impl<K> SingleFlightLeader<'_, K>
where
    K: Eq + Hash,
{
    fn complete(&mut self) {
        let Some(key) = self.key.take() else {
            return;
        };

        let mut in_flight = self.owner.in_flight.lock();
        let should_remove = in_flight
            .get(&key)
            .is_some_and(|state| Arc::ptr_eq(state, &self.state));
        if should_remove {
            in_flight.remove(&key);
        }
        drop(in_flight);

        let mut completed = self.state.completed.lock();
        *completed = true;
        drop(completed);
        self.state.completed_cv.notify_all();
    }
}

/// Wait permit for callers that joined an existing single-flight build.
#[derive(Debug)]
pub(crate) struct SingleFlightWaiter {
    state: Arc<FlightState>,
}

impl SingleFlightWaiter {
    /// Waits until the in-flight leader completes.
    pub(crate) fn wait(self) {
        let mut completed = self.state.completed.lock();
        while !*completed {
            self.state.completed_cv.wait(&mut completed);
        }
    }
}

#[derive(Debug)]
struct FlightState {
    completed: Mutex<bool>,
    completed_cv: Condvar,
}

impl Default for FlightState {
    fn default() -> Self {
        Self {
            completed: Mutex::new(false),
            completed_cv: Condvar::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    use super::{KeyedSingleFlight, SingleFlightPermit};

    #[test]
    fn same_key_callers_join_existing_leader() {
        let single_flight = Arc::new(KeyedSingleFlight::<u32>::default());
        let leader = match single_flight.acquire(7) {
            SingleFlightPermit::Leader(leader) => leader,
            SingleFlightPermit::Waiter(_) => panic!("first caller should lead"),
        };

        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        let mut threads = Vec::new();
        for _ in 0..4 {
            let single_flight = single_flight.clone();
            let ready_tx = ready_tx.clone();
            let done_tx = done_tx.clone();
            threads.push(std::thread::spawn(move || {
                let waiter = match single_flight.acquire(7) {
                    SingleFlightPermit::Waiter(waiter) => waiter,
                    SingleFlightPermit::Leader(_) => panic!("same key should join the leader"),
                };
                ready_tx.send(()).unwrap();
                waiter.wait();
                done_tx.send(()).unwrap();
            }));
        }
        drop(ready_tx);
        drop(done_tx);

        for _ in 0..4 {
            ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        }

        drop(leader);

        for _ in 0..4 {
            done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        }
        for thread in threads {
            thread.join().unwrap();
        }
    }

    #[test]
    fn different_keys_can_lead_concurrently() {
        let single_flight = Arc::new(KeyedSingleFlight::<u32>::default());
        let leader = match single_flight.acquire(1) {
            SingleFlightPermit::Leader(leader) => leader,
            SingleFlightPermit::Waiter(_) => panic!("first key should lead"),
        };
        let (tx, rx) = mpsc::channel();

        let worker = {
            let single_flight = single_flight.clone();
            std::thread::spawn(move || {
                let is_leader = matches!(single_flight.acquire(2), SingleFlightPermit::Leader(_));
                tx.send(is_leader).unwrap();
            })
        };

        assert!(rx.recv_timeout(Duration::from_secs(1)).unwrap());
        drop(leader);
        worker.join().unwrap();
    }

    #[test]
    fn dropped_leader_wakes_waiters_and_allows_retry() {
        let single_flight = Arc::new(KeyedSingleFlight::<u32>::default());
        let leader = match single_flight.acquire(3) {
            SingleFlightPermit::Leader(leader) => leader,
            SingleFlightPermit::Waiter(_) => panic!("first caller should lead"),
        };
        let (ready_tx, ready_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();

        let waiter_thread = {
            let single_flight = single_flight.clone();
            std::thread::spawn(move || {
                let waiter = match single_flight.acquire(3) {
                    SingleFlightPermit::Waiter(waiter) => waiter,
                    SingleFlightPermit::Leader(_) => panic!("same key should join the leader"),
                };
                ready_tx.send(()).unwrap();
                waiter.wait();
                done_tx.send(()).unwrap();
            })
        };

        ready_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        drop(leader);
        done_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        waiter_thread.join().unwrap();

        match single_flight.acquire(3) {
            SingleFlightPermit::Leader(_) => {}
            SingleFlightPermit::Waiter(_) => panic!("completed key should allow a new leader"),
        }
    }
}
