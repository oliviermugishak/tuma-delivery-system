//! Per-IP rate limiting for the auth doors (login, OTP request), built on
//! the `governor` crate's GCRA limiter — the vetted token-bucket variant.
//! Each door carries its configured per-minute budget as burst capacity:
//! ten rapid attempts pass, the eleventh is denied, and after the burst
//! the door admits one attempt a minute — stricter against grinding than
//! a sustained ten-per-minute drip, which would refund attempts back
//! mid-burst at password-hashing pace.
//!
//! One keyed limiter per door, IP-addressed, in memory — no shared
//! state: V1 has one server process, and the doors are the only
//! endpoints where unlimited guesses turn into password grinding. A
//! periodic sweep drops addresses the GCRA can no longer distinguish
//! from a fresh bucket, bounding memory against IP rotation.
//!
//! Fail-open everywhere the limiter itself might misfire: a request with
//! no `ConnectInfo` (no IP fact) passes, and the config switch disables
//! the whole thing. Non-auth routes never touch the buckets.

use core::num::NonZeroU32;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use governor::RateLimiter as Governor;
use governor::clock::DefaultClock;
use governor::state::keyed::DefaultKeyedStateStore;

/// How often a sweep runs: the keyed store keeps no clock of its own, so
/// addresses seen once would otherwise sit in memory forever.
const SWEEP_EVERY: Duration = Duration::from_secs(5 * 60);

/// One door's limiter: GCRA state per IP in a mutex-wrapped map.
type DoorStore = Governor<IpAddr, DefaultKeyedStateStore<IpAddr>, DefaultClock>;

/// Which door a request is knocking on. Each has its own budget.
#[derive(Debug, Clone, Copy)]
pub enum DoorClass {
    Login,
    Otp,
}

/// The two doors' limiters, shared behind `Arc` in `AppState`.
#[derive(Debug)]
pub struct RateLimiter {
    login: DoorStore,
    otp: DoorStore,
    last_sweep: Mutex<Instant>,
}

impl RateLimiter {
    pub fn new(login_per_minute: u32, otp_per_minute: u32) -> Self {
        Self {
            login: Governor::keyed(quota(login_per_minute)),
            otp: Governor::keyed(quota(otp_per_minute)),
            last_sweep: Mutex::new(Instant::now()),
        }
    }

    /// Whether one more request of `class` from `ip` may pass right now.
    /// `true` = admit (and spend one cell), `false` = the budget is spent.
    pub fn check(&self, class: DoorClass, ip: IpAddr) -> bool {
        self.sweep_if_due();
        let admitted = match class {
            DoorClass::Login => self.login.check_key(&ip),
            DoorClass::Otp => self.otp.check_key(&ip),
        }
        .is_ok();
        if !admitted {
            tracing::debug!(%ip, ?class, "auth door rate-limited an address");
        }
        admitted
    }

    /// Sweep when due: drop keys whose GCRA state is indistinguishable
    /// from a fresh bucket — the housekeeping the hand-rolled window did
    /// inline. One timestamp check per request; the O(keys) sweep itself
    /// runs at most every [`SWEEP_EVERY`].
    fn sweep_if_due(&self) {
        let mut last = self.last_sweep.lock().expect("sweep clock poisoned");
        if last.elapsed() < SWEEP_EVERY {
            return;
        }
        *last = Instant::now();
        drop(last);
        self.login.retain_recent();
        self.otp.retain_recent();
    }
}

/// A door budget as a GCRA quota: a burst of `per_minute` attempts,
/// then one attempt per minute. (Replenishing on the per-minute
/// interval instead — one cell every 60/N seconds — would drip
/// attempts back mid-burst, letting a slow-paced hammerer through the
/// cap the budget names.) A zero budget clamps to one — the crate
/// can't express zero, and the honest off-switch is `enabled`.
fn quota(per_minute: u32) -> governor::Quota {
    let burst = NonZeroU32::new(per_minute).unwrap_or(NonZeroU32::MIN);
    governor::Quota::with_period(Duration::from_secs(60))
        .expect("a 60-second period is nonzero")
        .allow_burst(burst)
}
