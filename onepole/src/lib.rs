/*
MIT License

Copyright (c) 2021 DGriffin91

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
*/

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
    }
}

impl Default for OnePoleModel {
    fn default() -> Self {
        Self {
            gain: 1.0,
            kind: 1.0,
            freq: 1000.0,
        }
    }
}


#[derive(Clone, Copy, Debug)]
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
            1 | 2 | 5 | _ => {
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
        }

        OnePoleCoeffs { a, g, a1 }
    }
}

#[derive(Clone, Copy, Debug)]
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

    fn process(&mut self, input: f64) -> f64 {
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
    filter_l: OnePoleFilter,
    filter_r: OnePoleFilter,
    sample_rate: f64,
}

impl Plugin for OnePole {
    const NAME: &'static str = "basic one pole filter";
    const PRODUCT: &'static str = "basic one pole filter";
    const VENDOR: &'static str = "DGriffin91";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = OnePoleModel;

    #[inline]
    fn new(sample_rate: f32, model: &OnePoleModel) -> Self {
        OnePole {
            filter_l: OnePoleFilter::new(
                model.kind as u8,
                sample_rate as f64,
                model.freq as f64,
                model.gain as f64,
            ),
            filter_r: OnePoleFilter::new(
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
            self.filter_l.coeffs = OnePoleCoeffs::new(
                kind,
                self.sample_rate,
                model.freq[i] as f64,
                model.gain[i] as f64,
            );
            self.filter_l.kind = kind;
            self.filter_r.coeffs = self.filter_l.coeffs;
            self.filter_r.kind = kind;

            let l = input[0][i] as f64;
            let r = input[1][i] as f64;

            let l = self.filter_l.process(l);
            let r = self.filter_r.process(r);

            output[0][i] = l as f32;
            output[1][i] = r as f32;
        }
    }
}

baseplug::vst2!(OnePole, b"tAbE");
