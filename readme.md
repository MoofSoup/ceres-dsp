# Ceres DSP Framework
Ceres DSP is an ergonomic, fast framework for writing DSP code in Rust. Currently, it's design centers on writing synthesizers ergonomically, but a lot of the ideas transfer to other DSP domains, and it will be expanded in the future. Here is an example of a Sawtooth Oscillator, that reads it's parameters with the 'use_paramaters::<T>()` hook.
```rust

#[parameter]
struct SawtoothOsc {
    frequency: f32,
    amplitude: f32,
}

fn sawtooth_osc(builder: &mut Builder) -> ComponentFn {
    let osc_handle = builder.use_state::<SawOscState>();
    let control_handle = builder.use_state::<ControlState>();
    let params_handle = builder.use_parameters::<SawtoothOsc>();
    
    Box::new(move |state, _input, output, sample_rate| {

        let osc_state = state.get_mut(&osc_handle);
        let control = state.get(&control_handle);
        let params = state.get_parameters(&params_handle);

        
        

        for (i, sample) in output.iter_mut().enumerate() {
            let SawtoothOsc { frequency, amplitude } = params[i];
            let mod_freq = frequency_modulator_to_hz(frequency);

            let freq = if mod_freq > 20.0 { mod_freq } else { 261.63 };

            let saw_wave = osc_state.phase * 2.0 - 1.0;
            *sample = saw_wave * amplitude;
            
            osc_state.phase += freq / sample_rate;
            if osc_state.phase >= 1.0 { osc_state.phase -= 1.0; }
    

        }
    })
}
```
