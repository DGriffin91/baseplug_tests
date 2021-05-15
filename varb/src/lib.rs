#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use serde::{Deserialize, Serialize};

use baseplug::{Plugin, ProcessContext};
mod comp;
mod svf;
mod units;

fn setup_logging() {
    let log_folder = ::dirs::home_dir().unwrap().join("tmp");

    let _ = ::std::fs::create_dir(log_folder.clone());

    let log_file = ::std::fs::File::create(log_folder.join("DGVerb.log")).unwrap();

    let log_config = ::simplelog::ConfigBuilder::new()
        .set_time_to_local(true)
        .build();

    let _ = ::simplelog::WriteLogger::init(simplelog::LevelFilter::Info, log_config, log_file);

    ::log_panics::init();

    ::log::info!("init");
}

baseplug::model! {
    #[derive(Debug, Serialize, Deserialize)]
    struct VerbPlugModel {

        #[model(min = 0.0, max = 1.0)]
        #[parameter(name = "Mix", unit = "Generic",
            gradient = "Linear")]
        mix: f32,

        #[model(min = 0.0001, max = 1000.0)]
        #[parameter(name = "Delay Size", unit = "Generic",
            gradient = "Linear")]
        delay_size: f32,

        #[model(min = 0.0, max = 1.5)]
        #[parameter(name = "Delay Delta", unit = "Generic",
            gradient = "Linear")]
        delay_delta: f32,

        #[model(min = 0.0, max = 1.5)]
        #[parameter(name = "Decay Init", unit = "Generic",
            gradient = "Linear")]
        decay_init: f32,

        #[model(min = 0.0, max = 1.5)]
        #[parameter(name = "Decay Delta", unit = "Generic",
            gradient = "Linear")]
        decay_delta: f32,

        #[model(min = 0.0, max = 64.0)]
        #[parameter(name = "Iterations", unit = "Generic",
            gradient = "Linear")]
        iterations: f32,

        #[model(min = -48.0, max = 48.0)]
        #[parameter(name = "Out Gain", unit = "Decibels",
            gradient = "Power(1.0)")]
        out_gain: f32,

    }
}

impl Default for VerbPlugModel {
    fn default() -> Self {
        Self {
            mix: 1.0,
            delay_size: 0.2,
            delay_delta: 0.9,
            decay_init: 0.9,
            decay_delta: 0.9,
            iterations: 16.0,
            out_gain: 1.0,
        }
    }
}

const MAX_BUFFER_LENGTH: usize = 960000;
const ITERATIONS: usize = 64;

fn mix(x: f64, y: f64, a: f64) -> f64 {
    x * (1.0 - a) + y * a
}

#[derive(Debug, Clone)]
pub struct VerbUnit {
    buffers: Vec<Vec<f64>>,
    delay: usize,
}

impl VerbUnit {
    pub fn new() -> VerbUnit {
        VerbUnit {
            buffers: vec![vec![0.0f64; MAX_BUFFER_LENGTH]; ITERATIONS],
            delay: 1000,
        }
    }

    pub fn process(
        &mut self,
        x: f64,
        delay_size: usize,
        delay_delta: f64,
        decay_init: f64,
        decay_delta: f64,
        iterations: usize,
        n: usize,
    ) -> f64 {
        let mut x = x;
        let mut decay = decay_init;
        self.delay = delay_size;
        for i in 0..iterations {
            x = self.line1(x, i, n, decay);
            decay *= decay_delta;
            self.delay = (delay_delta * self.delay as f64) as usize;
        }
        x
    }

    pub fn get(&self, buffer_num: usize, position: usize) -> f64 {
        self.buffers[buffer_num][position % MAX_BUFFER_LENGTH.min(self.delay).max(1)]
    }

    pub fn set(&mut self, buffer_num: usize, position: usize, value: f64) {
        self.buffers[buffer_num][position % MAX_BUFFER_LENGTH.min(self.delay).max(1)] = value;
    }

    pub fn line1(&mut self, x: f64, buffer_idx: usize, n: usize, gain: f64) -> f64 {
        let back = gain * self.get(buffer_idx, n + self.delay);
        self.set(buffer_idx, n, x + back);
        self.get(buffer_idx, n) + (-gain * x)
    }

    pub fn line2(&mut self, x: f64, buffer_idx: usize, n: usize, gain: f64) -> f64 {
        let back = gain * self.get(buffer_idx, n + self.delay);
        self.set(buffer_idx, n, x + back);
        (1.0 - gain * gain) * self.get(buffer_idx, n) + (-gain * x)
    }
}

struct VerbPlug {
    verbs: [VerbUnit; 2],
    sample_rate: f64,
    n: usize,
}

impl Plugin for VerbPlug {
    const NAME: &'static str = "Varb";
    const PRODUCT: &'static str = "Varb";
    const VENDOR: &'static str = "DGriffin";

    const INPUT_CHANNELS: usize = 2;
    const OUTPUT_CHANNELS: usize = 2;

    type Model = VerbPlugModel;

    #[inline]
    fn new(sample_rate: f32, _model: &VerbPlugModel) -> Self {
        setup_logging();
        VerbPlug {
            verbs: [VerbUnit::new(), VerbUnit::new()],
            sample_rate: sample_rate as f64,
            n: 0,
        }
    }

    #[inline]
    fn process(&mut self, model: &VerbPlugModelProcess, ctx: &mut ProcessContext<Self>) {
        let input = &ctx.inputs[0].buffers;
        let output = &mut ctx.outputs[0].buffers;
        for i in 0..ctx.nframes {
            let mix_amnt = model.mix[i] as f64;
            let delay_size = ((model.delay_size[i] as f64 * self.sample_rate) / 1000.0) as usize;
            let delay_delta = model.delay_delta[i] as f64;
            let decay_init = model.decay_init[i] as f64;
            let decay_delta = model.decay_delta[i] as f64;
            let iterations = model.iterations[i] as usize;
            let out_gain = model.out_gain[i] as f64;
            let in_l = input[0][i] as f64;
            let in_r = input[1][i] as f64;

            let l = self.verbs[0].process(
                in_l,
                delay_size,
                delay_delta,
                decay_init,
                decay_delta,
                iterations,
                self.n,
            );
            let r = self.verbs[1].process(
                in_r,
                delay_size,
                delay_delta,
                decay_init,
                decay_delta,
                iterations,
                self.n,
            );

            let l = l * out_gain;
            let r = r * out_gain;

            let l = mix(in_l, l, mix_amnt);
            let r = mix(in_r, r, mix_amnt);

            output[0][i] = l as f32;
            output[1][i] = r as f32;
            self.n = (self.n + 1) % MAX_BUFFER_LENGTH;
        }
    }
}

baseplug::vst2!(VerbPlug, b"tAnF");
