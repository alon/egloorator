pub struct Silence {
    // output
    silent: bool,

    // state changes per sample
    avg_rms: f64, // running average computation
    silent_current: i64, // time there has been silence

    // parameters (constant since construction)
    silent_period: i64,
    become_silent_threshold: f64,
    become_active_threshold: f64, // hysteresis needs these two to be different
    average_period: i64,

    // debug
    cycle: i64
}


impl Silence {

    pub fn new(silent_threshold: f64, active_threshold: f64, silent_period: i64, average_period: i64) -> Silence {
        Silence {
            become_active_threshold: active_threshold,
            become_silent_threshold: silent_threshold,
            silent_period: silent_period,
            average_period: average_period,
            silent: true,
            avg_rms: silent_threshold,
            silent_current: 0,

            // debug
            cycle: 0,
        }
    }

    pub fn input(&self, rms: f64) -> Silence {
        //println!("pre silent: cycle {} avg_rms {} silent {} silent_current {} ( S->A {}   A->S {})", self.cycle, self.avg_rms, self.silent, self.silent_current, self.become_active_threshold, self.become_silent_threshold);
        let silence_avg_f64 = self.average_period as f64;
        let silence_avg_minus_1_f64 = (self.average_period - 1) as f64;
        let avg_rms = rms / silence_avg_f64 + silence_avg_minus_1_f64 / silence_avg_f64 * self.avg_rms;
        let is_silence = avg_rms < match self.silent {
            true => self.become_active_threshold,
            false => self.become_silent_threshold
        };
        let silent_current = match is_silence {
            true => self.silent_current + 1,
            false => 0
        };
        let silent = match self.silent {
            true => is_silence,
            false => is_silence && silent_current >= self.silent_period
        };
        //println!("post silent: cycle {} avg_rms {} silent {} silent_current {} | is_silence {}", self.cycle + 1, avg_rms, silent, silent_current, is_silence);
        Silence {
            avg_rms: avg_rms,
            silent_current: silent_current,
            silent : silent,
            cycle : self.cycle + 1,
            .. *self
        }
    }

    pub fn output(&self) -> bool {
        self.silent
    }
}

#[cfg(test)]
mod tests {
    use super::Silence;

    const LIMIT_TALK: f64 = 2.0f64;
    const LIMIT_SILENCE: f64 = 1.0f64;
    const SILENCE_COUNT: i64 = 2;
    const AVERAGE_COUNT: i64 = 1;

    #[test]
    fn test_silence() -> ()
    {
        for (i, o) in vec![
            (vec![], vec![]),
            (vec![LIMIT_TALK - 0.01], vec![true]),
            (vec![LIMIT_TALK], vec![false]),
            (vec![LIMIT_TALK, LIMIT_SILENCE - 0.01, LIMIT_SILENCE - 0.01], vec![false, false, true])
        ] {
            test_silence_helper(i, o);
        }
    }

    fn test_silence_helper(inp: Vec<f64>, outp: Vec<bool>) -> () {
        let mut s = Silence::new(LIMIT_SILENCE, LIMIT_TALK, SILENCE_COUNT, AVERAGE_COUNT);
        let mut i = 0;

        for (rms, expected) in inp.iter().zip(outp.iter()) {
            s = s.input(*rms);
            // need an assert_eq_message!
            if s.output() != *expected {
                println!("{:?} => {:?} failed, step {}: expected {}, got {}", inp, outp, i, *expected, s.output());
                assert!(false)
            }
            i += 1;
        }
    }
}
