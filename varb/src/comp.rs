use crate::units::Units;
use std::f64::consts::PI;

#[derive(Debug, Clone, Copy)]
pub struct Comp {
    prev_env: f64,
    cte_attack: f64,
    cte_release: f64,
    thrlin: f64,
    ratio: f64,
}

impl Comp {
    pub fn new(threshold: f64, attack: f64, release: f64, sample_rate: f64, ratio: f64) -> Comp {
        let mut new_comp = Comp {
            prev_env: 1.0,
            cte_attack: 0.0,
            cte_release: 0.0,
            thrlin: 0.0,
            ratio: 0.0,
        };
        new_comp.update(threshold, attack, release, sample_rate, ratio);
        new_comp
    }

    pub fn update(
        &mut self,
        threshold: f64,
        attack: f64,
        release: f64,
        sample_rate: f64,
        ratio: f64,
    ) {
        self.thrlin = threshold.db_to_lin();
        self.cte_attack = (-2.0 * PI * 1000.0 / attack / sample_rate).exp();
        self.cte_release = (-2.0 * PI * 1000.0 / release / sample_rate).exp();
        self.ratio = ratio;
    }

    pub fn process(&mut self, detector_input: f64) -> f64 {
        let cte = if detector_input >= self.prev_env {
            self.cte_attack
        } else {
            self.cte_release
        };
        let env = detector_input + cte * (self.prev_env - detector_input);
        self.prev_env = env;
        // Compressor transfer function
        if env <= self.thrlin {
            1.0
        } else {
            (env / self.thrlin).powf(1.0 / self.ratio - 1.0)
        }
    }
}
