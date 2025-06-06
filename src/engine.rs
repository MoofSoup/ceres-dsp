// build_synth.rs is the entrypoint for the audio engine
// the AudioEngine object owns and manages the audio engine thread, and contains all the cpal logic.
use crate::core::*;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam::channel::Sender;

pub struct Engine<E: Clone + Copy + Send + 'static> {
    pub tx: Sender<E>,
    stream: cpal::platform::Stream,
}

impl<E> Engine<E> 
where 
    E: Clone + Copy + Send + 'static,
{
    pub fn new<F>(f: F) -> Self 
    where
        F: for<'a> FnOnce(Builder<E>) -> Runtime<E>,
    {
        let (event_bus, builder) = new::<E>();
        let EventBus{tx, rx} = event_bus;
        
        // cpal setup
        let host = cpal::default_host();
        let device = host.default_output_device()
            .ok_or("no output device available").unwrap();
        let config = device.default_output_config().unwrap();
        let sample_rate = config.sample_rate().0 as f32;
        let mut runtime = f(builder);

        Engine {
            tx,
            stream: device.build_output_stream(
                &config.into(),
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {

                    let input = vec![0.0; data.len()];

                    for (input_chunk, output_chunk) in input.chunks(256).zip(data.chunks_mut(256)) {
                        if let Ok(event) = rx.try_recv() {
                        runtime.tick(sample_rate, Some(event), &input_chunk, output_chunk);
                        } else {
                            runtime.tick(sample_rate, None, &input_chunk, output_chunk)
                        }
                    }
                },
                |err| eprintln!("Audio stream error: {}", err),
                None,
            ).unwrap(),
        }
    }

    pub fn run(&self) {
        self.stream.play().unwrap();
    }
}