#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use serde::{Deserialize, Serialize};

use baseplug::{Plugin, ProcessContext};
use units::map_to_freq;

mod comp;
mod svf;
mod units;

use crate::svf::{SVFCoefficients, Type, SVF};

use crate::comp::Comp;

fn setup_logging() {
    let log_folder = ::dirs::home_dir().unwrap().join("tmp");

    let _ = ::std::fs::create_dir(log_folder.clone());

    let log_file = ::std::fs::File::create(log_folder.join("DynSat.log")).unwrap();

    let log_config = ::simplelog::ConfigBuilder::new()
        .set_time_to_local(true)
        .build();

    let _ = ::simplelog::WriteLogger::init(simplelog::LevelFilter::Info, log_config, log_file);

    ::log_panics::init();

    ::log::info!("init");
}

const FILTER_COUNT: usize = 16;

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct DynSatModel {
        #[model(min = -12.0, max = 96.0)]
        #[parameter(name = "Gain", unit = "Decibels",
            gradient = "Power(1.0)")]
        gain: f32,
        #[model(min = -96.0, max = 12.0)]
        #[parameter(name = "Out Gain", unit = "Decibels",
            gradient = "Power(1.0)")]
        out_gain: f32,
        #[model(min = 1.0, max = 10.0)]
        #[parameter(name = "Mode", unit = "Generic",
            gradient = "Linear")]
        mode: f32
    }
}

impl Default for DynSatModel {
    fn default() -> Self {
        Self {
            // "DynSat" is converted from dB to coefficient in the parameter handling code,
            // so in the model here it's a coeff.
            // -0dB == 1.0
            gain: 1.0,
            out_gain: 1.0,
            mode: 1.0,
        }
    }
}

struct DynSat {
    svfs: [[SVF<f64>; 2]; FILTER_COUNT],
    comps: [[Comp; 2]; FILTER_COUNT],
}

impl Plugin for DynSat {
    const NAME: &'static str = "DynSat";
    const PRODUCT: &'static str = "DynSat";
    const VENDOR: &'static str = "DGriffin";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = DynSatModel;

    #[inline]
    fn new(sample_rate: f32, _model: &DynSatModel) -> Self {
        setup_logging();
        let coeffs =
            SVFCoefficients::<f64>::from_params(Type::BandPass, sample_rate as f64, 100.0, 1.0)
                .unwrap();
        let mut svfs = [[SVF::<f64>::new(coeffs); 2]; FILTER_COUNT];
        let len = svfs.len();
        for (i, svf) in svfs.iter_mut().enumerate() {
            let hz = map_to_freq(i as f32 / (len - 1) as f32);
            ::log::info!("{}", hz);
            let coeffs2 = SVFCoefficients::<f64>::from_params(
                Type::BandPass,
                sample_rate as f64,
                hz as f64,
                0.70710678118654757f64 * 2.0,
            )
            .unwrap();
            svf[0].update_coefficients(coeffs2);
            svf[1].update_coefficients(coeffs2);
        }

        let comps = [[Comp::new(0.0, 10.0, 20.0, 48000.0, 5.0); 2]; FILTER_COUNT];
        DynSat { svfs, comps }
    }

    #[inline]
    fn process(&mut self, model: &DynSatModelProcess, ctx: &mut ProcessContext<Self>) {
        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;
        for i in 0..ctx.nframes {
            let mode = model.mode[i] as u8;
            let gain = model.gain[i] as f64;
            let out_gain = model.out_gain[i] as f64;
            let l = input[0][i] as f64;
            let r = input[1][i] as f64;
            let mut l_out = 0.0 as f64;
            let mut r_out = 0.0 as f64;

            if mode == 1 || mode == 2 {
                let l_a = l * gain;
                let r_a = r * gain;
                for (svf, comp) in self.svfs.iter_mut().zip(&mut self.comps) {
                    let mut l_band = svf[0].run(l_a);
                    let mut r_band = svf[1].run(r_a);
                    let cv_l = comp[0].process(l_band.abs() as f64);
                    let cv_r = comp[1].process(r_band.abs() as f64);
                    l_band *= cv_l;
                    r_band *= cv_r;
                    if mode == 1 {
                        l_band = (l_band * gain).tanh();
                        r_band = (r_band * gain).tanh();
                        l_band /= cv_l;
                        r_band /= cv_r;
                    }
                    l_out += l_band;
                    r_out += r_band;
                }
                l_out /= (self.svfs.len() as f64) * 0.25;
                r_out /= (self.svfs.len() as f64) * 0.25;
                l_out *= out_gain;
                r_out *= out_gain;
            } else if mode == 3 {
                let l_a = l * gain;
                let r_a = r * gain;
                let cv_l = self.comps[0][0].process(l_a.abs() as f64);
                let cv_r = self.comps[0][1].process(r_a.abs() as f64);
                l_out = l_a * cv_l * out_gain;
                r_out = r_a * cv_r * out_gain;
            } else {
                l_out = (l * gain).tanh() * out_gain;
                r_out = (r * gain).tanh() * out_gain;
            }

            output[0][i] = l_out as f32;
            output[1][i] = r_out as f32;
        }
    }
}

baseplug::vst2!(DynSat, b"tAnE");
