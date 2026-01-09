pub mod pulse;

#[cfg(test)]
mod tests {
    use crate::pulse::*;

    #[test]
    fn soundness_contract() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;

        // send one heartbeat at t = 0
        hb_fsm.step(0, Hb::Seen, T, W);
        assert_eq!(hb_fsm.state(), State::Alive);

        // advance past timeout without heartbeat
        hb_fsm.step(T + 1, Hb::NotSeen, T, W);

        // must not be alive
        assert_ne!(hb_fsm.state(), State::Alive);
        assert_eq!(hb_fsm.state(), State::Dead);
    }

    #[test]
    fn liveness_contract() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;

        // send one heartbeat
        hb_fsm.step(0, Hb::Seen, T, W);
        assert_eq!(hb_fsm.state(), State::Alive);

        // simulate time passing, no heartbeats;
        // must reach State::Dead by T + P (P = polling interval)
        for t in (100u64..=T + 100u64).step_by(100) {
            hb_fsm.step(t, Hb::NotSeen, T, W);
        }

        assert_eq!(hb_fsm.state(), State::Dead);
    }

    #[test]
    fn stability_contract() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;

        // send heartbeats every 100ms, 
        // verify no spurious State::Dead
        for t in (0..=10000u64).step_by(100) {
            hb_fsm.step(t, Hb::Seen, T, W);

            assert_eq!(hb_fsm.state(), State::Alive);
        }
    }

    #[test]
    fn boundary_conditions() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;

        // test at T - 1; should be State::Alive
        hb_fsm.step(0, Hb::Seen, T, W);
        hb_fsm.step(T - 1, Hb::NotSeen, T, W);
        assert_eq!(hb_fsm.state(), State::Alive);

        // reset and test at exactly T; should be State::Alive
        hb_fsm.init(0);
        hb_fsm.step(0, Hb::Seen, T, W);
        hb_fsm.step(T, Hb::NotSeen, T, W);
        assert_eq!(hb_fsm.state(), State::Alive);

        // test at T + 1; should be State::Dead
        hb_fsm.init(0);
        hb_fsm.step(0, Hb::Seen, T, W);
        hb_fsm.step(T + 1, Hb::NotSeen, T, W);
        assert_eq!(hb_fsm.state(), State::Dead);
    }

    #[test]
    fn invariants_hold() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;

        verify_invariants(&hb_fsm);

        // test through various state transitions
        hb_fsm.step(0, Hb::Seen, T, W);
        verify_invariants(&hb_fsm);

        hb_fsm.step(500, Hb::NotSeen, T, W);
        verify_invariants(&hb_fsm);

        hb_fsm.step(T + 1, Hb::NotSeen, T, W);
        verify_invariants(&hb_fsm);

        hb_fsm.step(T + 2, Hb::Seen, T, W); // recovery
        verify_invariants(&hb_fsm);
    }

    #[test]
    fn clock_corruption() {
        let mut hb_fsm = HbFsm::new(1000);
        let T = 1000u64;
        let W = 0u64;

        hb_fsm.step(1000, Hb::Seen, T, W);
        assert_eq!(hb_fsm.state(), State::Alive);

        // simulate clock jump backwards;
        // now < last_hb by huge amount, making age > 2^63
        hb_fsm.step(500, Hb::NotSeen, T, W);

        // age = 500 - 1000 = huge number due to unsigned wrap
        // should trigger fault_time and State::Dead
        assert_eq!(hb_fsm.state(), State::Dead);
        assert_eq!(hb_fsm.faulted(), true);
    }

    use std::sync::{Arc, Barrier};

    #[test]
    fn reentrancy_detection() {
        // force a date race
        static mut hb_fsm: HbFsm = HbFsm::new(0);
        static T: u64 = 1000u64;
        static W: u64 = 0u64;
       
        unsafe {
            let m = &raw mut hb_fsm;
            (*m).step(0, Hb::Seen, T, W);
        }

        let n_threads = 8;
        let barrier = Arc::new(Barrier::new(n_threads + 1));

        let mut handles = Vec::new();
        for _ in 0..n_threads {
            let b = barrier.clone();
            handles.push(std::thread::spawn(move || {
                b.wait();

                for _ in 0..100_000 {
                    unsafe {
                        let m = &raw mut hb_fsm;
                        (*m).step(0, Hb::Seen, T, W);
                    }
                    std::thread::yield_now();
                }
            }));
        }

        barrier.wait();

        for h in handles {
            h.join().unwrap();
        }

        unsafe {
            let m = &raw mut hb_fsm;
            // should detect reentry and failsafe
            assert_eq!((*m).state(), State::Dead);
            assert_eq!((*m).faulted(), true);
        }
    }

    use rand::Rng;

    #[test]
    fn test_fuzz() {
        let mut hb_fsm = HbFsm::new(0);
        let T = 1000u64;
        let W = 0u64;
        let mut now = 0u64;

        let mut rng = rand::rng();

        for i in 0..100000 {
            let hb = Hb::from_bool(rng.random_bool(0.5));

            now += rng.gen_range(1..=500);

            let old_state: State = hb_fsm.state();
            let old_last_hb: u64 = hb_fsm.last_hb();

            hb_fsm.step(now, hb.clone(), T, W);

            verify_invariants(&hb_fsm);

            if hb_fsm.state() == State::Alive {
                let age: u64 = now - hb_fsm.last_hb();
                assert!(age <= T);
                assert_eq!(hb_fsm.has_evidence(), true);
            }

            if hb_fsm.has_evidence() && hb == Hb::NotSeen {
                let age: u64 = now - hb_fsm.last_hb();
                if age > T {
                    assert_eq!(hb_fsm.state(), State::Dead);
                }
            }
        }
    }

    // helper functions
    //
    fn verify_invariants(m: &HbFsm) {
        // check for valid state
        assert!(m.state() == State::Unknown ||
                m.state() == State::Alive   ||
                m.state() == State::Dead);

        // State::Alive requires evidence
        if m.state() == State::Alive {
            assert_eq!(m.has_evidence(), true);
        }

        // fault implies State::Dead
        if m.faulted() {
            assert_eq!(m.state(), State::Dead);
        }

        // assert not in step
        assert_eq!(m.in_step(), false);
    }
}   

