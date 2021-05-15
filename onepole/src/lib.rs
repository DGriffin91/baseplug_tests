#![allow(incomplete_features)]
#![feature(generic_associated_types)]

/*
Also, need to do bode plot.
https://ccrma.stanford.edu/~jos/svf/svf.pdf (Page 6)
https://www.earlevel.com/DigitalAudio/images/StateVarBlock.gif
http://www.willpirkle.com/Downloads/AN-4VirtualAnalogFilters.pdf (Page 6)
*/

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use baseplug::{Plugin, ProcessContext};

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct OnePoleModel {
        #[model(min = -6.0, max = 6.0)]
        #[parameter(name = "gain", unit = "Generic",
            gradient = "Linear")]
        gain: f32,

        #[model(min = 20.0, max = 20000.0)]
        #[parameter(name = "freq", unit = "Generic",
            gradient = "Power(10.0)")]
        freq: f32,

        #[model(min = 1.0, max = 10.0)]
        #[parameter(name = "kind", unit = "Generic",
            gradient = "Linear")]
        kind: f32,

        #[model(min = -2.0, max = 0.0)]
        #[parameter(name = "var", unit = "Generic",
            gradient = "Linear")]
        var: f32,
    }
}

impl Default for OnePoleModel {
    fn default() -> Self {
        Self {
            // "gain" is converted from dB to coefficient in the parameter handling code,
            // so in the model here it's a coeff.
            // -0dB == 1.0
            gain: 1.0,
            kind: 1.0,
            freq: 9000.0,
            var: 0.0,
        }
    }
}

struct OnePoleCoeffs {
    a: f64,
    g: f64,
    a1: f64,
}

impl OnePoleCoeffs {
    fn new(kind: u8, fs: f64, f0: f64, db_gain: f64) -> OnePoleCoeffs {
        let mut a = 0.0;
        let mut g = 0.0;
        let mut a1 = 0.0;

        match kind {
            1 | 2 | 5 => {
                // Low pass | High pass | All Pass
                a = 1.0;
                g = (PI * f0 / fs).tan();
                a1 = g / (1.0 + g);
            }
            3 => {
                // Low Shelf
                a = 10.0f64.powf(db_gain / 20.0);
                g = (PI * f0 / fs).tan() / (a).sqrt();
                a1 = g / (1.0 + g);
            }
            4 => {
                // High Shelf
                a = 10.0f64.powf(db_gain / 20.0);
                g = (PI * f0 / fs).tan() * (a).sqrt();
                a1 = g / (1.0 + g);
            }
            _ => {}
        }

        OnePoleCoeffs { a, g, a1 }
    }
}

struct OnePoleFilter {
    pub kind: u8,
    ic1eq: f64,
    pub coeffs: OnePoleCoeffs,
}

impl OnePoleFilter {
    fn new(kind: u8, fs: f64, f0: f64, db_gain: f64) -> OnePoleFilter {
        OnePoleFilter {
            kind,
            ic1eq: 0.0,
            coeffs: OnePoleCoeffs::new(kind, fs, f0, db_gain),
        }
    }

    fn process(&mut self, input: f64, var: f64) -> f64 {
        //http://www.willpirkle.com/Downloads/AN-4VirtualAnalogFilters.pdf (page 5)
        let v1 = self.coeffs.a1 * (input - self.ic1eq);
        let v2 = v1 + self.ic1eq;
        self.ic1eq = v2 + v1;

        // Low pass
        let mut m0 = 0.0;
        let mut m1 = 1.0;

        match self.kind {
            1 => {
                // Low pass
                m0 = 0.0;
                m1 = 1.0;
            }
            2 => {
                // High pass
                m0 = 1.0;
                m1 = -1.0;
            }
            3 => {
                // Low Shelf
                m0 = 1.0;
                m1 = self.coeffs.a - 1.0;
            }
            4 => {
                // High Shelf
                m0 = self.coeffs.a;
                m1 = 1.0 - self.coeffs.a;
            }
            5 => {
                // All pass
                m0 = 1.0;
                m1 = -2.0;
            }
            _ => {}
        }

        m0 * input + m1 * v2
    }
}

struct OnePole {
    filter_1: OnePoleFilter,
    sample_rate: f64,
}

impl Plugin for OnePole {
    const NAME: &'static str = "basic gain plug";
    const PRODUCT: &'static str = "basic gain plug";
    const VENDOR: &'static str = "spicy plugins & co";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = OnePoleModel;

    #[inline]
    fn new(sample_rate: f32, model: &OnePoleModel) -> Self {
        OnePole {
            filter_1: OnePoleFilter::new(
                model.kind as u8,
                sample_rate as f64,
                model.freq as f64,
                model.gain as f64,
            ),
            sample_rate: sample_rate as f64,
        }
    }

    #[inline]
    fn process(&mut self, model: &OnePoleModelProcess, ctx: &mut ProcessContext<Self>) {
        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;

        for i in 0..ctx.nframes {
            let kind = model.kind[i] as u8;
            self.filter_1.coeffs = OnePoleCoeffs::new(
                kind,
                self.sample_rate,
                model.freq[i] as f64,
                model.gain[i] as f64,
            );
            self.filter_1.kind = kind;

            let l = input[0][i] as f64;

            let l = self.filter_1.process(l, model.var[i] as f64);

            output[0][i] = l as f32;
        }
    }
}

baseplug::vst2!(OnePole, b"tAbE");
