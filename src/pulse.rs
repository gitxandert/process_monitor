#[derive(Copy, Clone, Debug, PartialEq)]
pub enum State {
    Unknown,
    Alive,
    Dead,
}

// more explicit hb flag
#[derive(Clone, PartialEq)]
pub enum Hb {
    NotSeen,
    Seen,
}

impl Hb {
    pub fn from_bool(b: bool) -> Self {
        match b {
            true => Self::Seen,
            false => Self::NotSeen,
        }
    }
}

use std::sync::atomic::{AtomicBool, Ordering};

pub struct HbFsm {
    state:          State,
    t_init:         u64,
    last_hb:        u64,
    have_hb:        bool,
    fault_time:     bool,
    fault_reentry:  bool,
    in_step:        AtomicBool,
}

impl HbFsm {
    pub const fn new(now: u64) -> Self {
        Self {
            state:          State::Unknown,
            t_init:         now,
            last_hb:        0u64,
            have_hb:        false,
            fault_time:     false,
            fault_reentry:  false,
            in_step:        AtomicBool::new(false),
        }
    }

    pub fn init(&mut self, now: u64) {
        self.state = State::Unknown;
        self.t_init = now;
        self.last_hb = 0u64;
        self.have_hb = false;
        self.fault_time = false;
        self.fault_reentry = false;
        self.in_step.store(false, Ordering::Relaxed);
    }

    pub fn step(&mut self, now: u64, hb: Hb, T: u64, W: u64) {
        // prevents threads from writing state
        // after some other thread has already faulted it
        if self.faulted() {
            return;
        }

        if self.in_step.swap(true, Ordering::AcqRel) {
            self.fault_reentry = true;
            self.state = State::Dead;
            return;
        }

        if hb == Hb::Seen {
            self.last_hb = now;
            self.have_hb = true;
        }

        if !self.have_hb {
            let a_init: u64 = Self::age_u64(&now, &self.t_init);
            if !Self::age_valid(&a_init) {
                self.fault_time = true;
                self.state = State::Dead;
            } else {
                self.state = State::Unknown;
            }
            self.in_step.store(false, Ordering::Release);
            return;
        }

        let a_hb: u64 = Self::age_u64(&now, &self.last_hb);
        if !Self::age_valid(&a_hb) {
            self.fault_time = true;
            self.state = State::Dead;
            self.in_step.store(false, Ordering::Release);
            return;
        }

        if a_hb > T {
            self.state = State::Dead;
        } else {
            self.state = State::Alive;
        }

        self.in_step.store(false, Ordering::Release);
    }

    #[inline]
    pub fn state(&self) -> State {
        self.state
    } 

    #[inline]
    pub fn has_evidence(&self) -> bool {
        self.have_hb
    }

    #[inline]
    pub fn last_hb(&self) -> u64 {
        self.last_hb
    }

    #[inline]
    pub fn faulted(&self) -> bool {
        self.fault_time || self.fault_reentry
    }

    #[inline]
    pub fn in_step(&self) -> bool {
        self.in_step.load(Ordering::Relaxed)
    }

    // private helpers for age calculation and validation
    #[inline]
    fn age_u64(now: &u64, then: &u64) -> u64 {
        now.wrapping_sub(*then)
    }

    #[inline]
    fn age_valid(age: &u64) -> bool {
        *age < (1u64 << 63)
    }
}
