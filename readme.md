# Ceres DSP
At Ceres DSP, we believe in making the impossible fun. Ceres DSP is an ergonomics-first, component based digital signal processing *framework.* Using the Ceres Runtime lets you focus on what matters: writing digital signal processors. 
## Core Features:
- `use_state<T>()` hook enables Ceres Runtime to manage processor state
- Component based processors are composable with `serial!()` and `parallel!()` macros
- `ceres::new<E>()` provides channel based api to send events to audio runtime
- Synth Engine provides cpal integration
- Modulators (event handlers, think envelopes if you speak synthesizer)
- `use_parameters<T>()` hook + `#[parameters]` proc macro

## The Future:
Currently, we are working on redoing our routing API to support fluent rerouting between components at runtime. We are also working on a feedback based drum synthesizer VST, inspired by SOPHIE. For updates, to contribute, or just to make cool projects and share your work, please join the official [Ceres Discord](https://discord.gg/QgVPEETetC)

## Example:

You can find the example code's repo [here](https://github.com/MoofSoup/hello-ceres). Alternatively:
```bash
git clone https://github.com/MoofSoup/hello-ceres.git
```

Here, we use Ceres' Synth Engine to set up a basic synthesizer:
```rust
// main.rs
#[derive(Clone, Copy)]
enum Event {
    midi([u8; 3])
}
fn main() {

    let engine = Engine::<Event>::new(|builder|{
        let runtime = builder.build(sawtooth);
        runtime
    });

    engine.run();
    println!("Audio Engine is running! Press Ctrl + C to stop!");
    std::thread::park();
    
}
```

Here is how our sawtooth oscillator is defined:

```rust

// this struct holds our oscillator's internal state
#[derive(Default)]
struct SawOscState {
	phase: f32,
}

pub fn sawtooth(builder: &mut Builder<Event>) -> ComponentFn<Event>{
	
	// Here, we get a handle to our state at build time
	// When the runtime eventually gets constructed, it will:
	//  - Create and manage an instance of the saw osc state struct
	//  - that is shared across componants
	// this means you have to create structs specific to each component's state
	let osc_state_handle = builder.use_state::<SawOscState>();
	
	Box::new(move |runtime, input, output, sample_rate|{
		
		// Now, we use our handle to access our state each tick
		let state = runtime.get_mut(&osc_state_handle);

		// Now, we write to our output buffer.
		// We use the phase  in our calculations, and update it
		// The use_state hook makes managing this state effortless.
		for (i, sample) in output.iter_mut().enumerate() {
		
			let freq = 261.63;
			let amplitude = 1.0;
			let saw_wave = state.phase * 2.0 - 1.0;
			*sample = saw_wave * amplitude;
			state.phase += freq / sample_rate;
			if state.phase >= 1.0 { state.phase -= 1.0; }
			
		}
	})
}
```

