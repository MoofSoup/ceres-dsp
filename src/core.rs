//! Core framework types and traits

use std::collections::HashMap;
use std::marker::PhantomData;
use std::any::{Any, TypeId};
use std::cell::UnsafeCell;
use crossbeam::channel::{Receiver, Sender, unbounded, TryRecvError};

pub const BUFFER_SIZE: usize = 256;

pub type ComponentFn<E> = Box<dyn FnMut(&mut Runtime<E>, &[f32], &mut [f32], f32) + Send>;

// === Event Bus ===
pub struct EventBus<E> {
    tx: Sender<E>,
    rx: Receiver<E>,
}

impl<E> EventBus<E> {
    fn new() -> Self {
        let (tx, rx) = unbounded();
        Self { tx, rx }
    }
    
    pub fn send(&self, event: E) -> Result<(), crossbeam::channel::SendError<E>> {
        self.tx.send(event)
    }
    
    pub fn sender(&self) -> Sender<E> {
        self.tx.clone()
    }
    
    fn try_recv_all(&self) -> Vec<E> {
        let mut events = Vec::new();
        while let Ok(event) = self.rx.try_recv() {
            events.push(event);
        }
        events
    }
}

impl<E> Clone for EventBus<E> {
    fn clone(&self) -> Self {
        Self {
            tx: self.tx.clone(),
            rx: self.rx.clone(),
        }
    }
}

// === Handles ===
#[derive(Copy, Clone)]
pub struct StateHandle<T> {
    pub(crate) slot: usize,
    _phantom: PhantomData<T>,
}

#[derive(Copy, Clone)]
pub struct ModulatorHandle<T> {
    pub(crate) slot: usize,
    _phantom: PhantomData<T>,
}

#[derive(Copy, Clone)]
pub struct ParameterHandle<T> {
    pub(crate) slot: usize,
    _phantom: PhantomData<T>,
}

// === Traits ===
pub trait Modulator<E>: Send + 'static {
    fn update(&mut self, sample_rate: f32, events: &[E]);
    fn get_value(&self, index: usize) -> f32;
}

pub trait Parameters: Default + Send + 'static {
    type Runtime<E>: ParameterRuntime<E> + Send;
    type Accessor<'a, E>;
    type Values: Copy;
    
    fn create_runtime<E>() -> Self::Runtime<E>;
    fn create_accessor<E>(runtime: &Self::Runtime<E>) -> Self::Accessor<'_, E>;
}

pub trait ParameterRuntime<E>: Send {
    fn update(&mut self, sources: &[Box<dyn Modulator<E>>]);
    fn route_parameter(&mut self, param_name: &str, source_index: usize, amount: f32);
}

// === Builder ===
pub struct Builder<E> {
    pub(crate) next_state_slot: usize,
    pub(crate) state_builders: Vec<Box<dyn FnOnce() -> Box<dyn Any + Send>>>,
    pub(crate) state_map: HashMap<TypeId, usize>,
    
    pub(crate) next_modulation_slot: usize,
    pub(crate) modulation_builders: Vec<Box<dyn FnOnce() -> Box<dyn ParameterRuntime<E>>>>,
    pub(crate) modulation_map: HashMap<TypeId, usize>,
    
    pub(crate) next_source_slot: usize,
    pub(crate) modulation_sources: Vec<Box<dyn Modulator<E>>>,
    pub(crate) source_map: HashMap<TypeId, usize>,
    
    _phantom: PhantomData<E>,
}

impl<E> Builder<E> {
    fn new() -> Self {
        Self {
            next_state_slot: 0,
            state_builders: Vec::new(),
            state_map: HashMap::new(),
            next_modulation_slot: 0,
            modulation_builders: Vec::new(),
            modulation_map: HashMap::new(),
            next_source_slot: 0,
            modulation_sources: Vec::new(),
            source_map: HashMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn use_state<T: Default + Send + 'static>(&mut self) -> StateHandle<T> {
        let type_id = TypeId::of::<T>();
        let slot = *self.state_map.entry(type_id).or_insert_with(|| {
            let slot = self.next_state_slot;
            self.next_state_slot += 1;
            self.state_builders.push(Box::new(|| Box::new(T::default())));
            slot
        });
        StateHandle { slot, _phantom: PhantomData }
    }
    
    pub fn use_parameters<T: Parameters>(&mut self) -> ParameterHandle<T> 
    where T::Runtime<E>: ParameterRuntime<E> + 'static {
        let type_id = TypeId::of::<T>();
        let slot = *self.modulation_map.entry(type_id).or_insert_with(|| {
            let slot = self.next_modulation_slot;
            self.next_modulation_slot += 1;
            self.modulation_builders.push(Box::new(|| Box::new(T::create_runtime::<E>())));
            slot
        });
        ParameterHandle { slot, _phantom: PhantomData }
    }
    
    pub fn use_modulator<T: Modulator<E> + Default>(&mut self) -> ModulatorHandle<T> {
        let type_id = TypeId::of::<T>();
        let slot = self.next_source_slot;
        self.next_source_slot += 1;
        
        self.modulation_sources.push(Box::new(T::default()));
        self.source_map.insert(type_id, slot);
        
        ModulatorHandle { slot, _phantom: PhantomData }
    }
    
    pub fn build<F>(self, f: F) -> Runtime<E> 
    where 
        F: FnOnce(&mut Builder<E>) -> ComponentFn<E>
    {
        let mut builder = self;
        let component = f(&mut builder);
        
        Runtime {
            states: builder.state_builders
                .into_iter()
                .map(|builder| UnsafeCell::new(builder()))
                .collect(),
            modulation_targets: builder.modulation_builders
                .into_iter()
                .map(|builder| UnsafeCell::new(builder()))
                .collect(),
            modulation_sources: UnsafeCell::new(builder.modulation_sources),
            component: UnsafeCell::new(component),
        }
    }
}

// === Runtime ===
pub struct Runtime<E: 'static> {
    pub(crate) states: Vec<UnsafeCell<Box<dyn Any + Send>>>,
    pub(crate) modulation_targets: Vec<UnsafeCell<Box<dyn ParameterRuntime<E>>>>,
    pub(crate) modulation_sources: UnsafeCell<Vec<Box<dyn Modulator<E>>>>,
    pub(crate) component: UnsafeCell<ComponentFn<E>>,
}

impl<E: 'static> Runtime<E> {
    pub fn get<T: 'static>(&self, handle: &StateHandle<T>) -> &T {
        unsafe {
            (*self.states[handle.slot].get()).downcast_ref().unwrap()
        }
    }
    
    pub fn get_mut<T: 'static>(&self, handle: &StateHandle<T>) -> &mut T {
        unsafe {
            (*self.states[handle.slot].get()).downcast_mut().unwrap()
        }
    }

    pub fn get_source_mut<T: Modulator<E> + 'static>(&self, handle: &ModulatorHandle<T>) -> &mut T {
        unsafe {
            let sources = &mut *self.modulation_sources.get();
            let boxed_modulator = &mut sources[handle.slot];
            &mut *(boxed_modulator.as_mut() as *mut dyn Modulator<E> as *mut T)
        }
    }

    pub fn route<S: 'static, T: Parameters + 'static>(
        &mut self, 
        source: ModulatorHandle<S>, 
        target: ParameterHandle<T>, 
        param: &str, 
        amount: f32
    ) {
        unsafe {
            let target_runtime = &mut *self.modulation_targets[target.slot].get();
            target_runtime.route_parameter(param, source.slot, amount);
        }
    }

    pub fn tick(&mut self, sample_rate: f32, events: &[E], input: &[f32], output: &mut [f32]) {
        unsafe {
            let sources = &mut *self.modulation_sources.get();
            for modulator in sources.iter_mut() {
                modulator.update(sample_rate, events);
            }
            
            let component = &mut *self.component.get();
            component(self, input, output, sample_rate);
        }
    }
    
    pub fn get_parameters<T: Parameters>(&self, handle: &ParameterHandle<T>) -> T::Accessor<'_, E> {
        unsafe {
            let sources = &*self.modulation_sources.get();
            
            let target_boxed = &mut *self.modulation_targets[handle.slot].get();
            let concrete_runtime = &mut *(target_boxed.as_mut() as *mut dyn ParameterRuntime<E> as *mut T::Runtime<E>);
            
            concrete_runtime.update(sources);
            T::create_accessor(concrete_runtime)
        }
    }
}

// === Main API ===
pub fn new<E: Clone + Send + 'static>() -> (EventBus<E>, Builder<E>) {
    (EventBus::new(), Builder::new())
}

// === Macros ===
#[macro_export]
macro_rules! parallel {
    ($(($weight:expr, $comp:expr)),+) => {
        |builder: &mut $crate::Builder<_>| -> $crate::ComponentFn<_> {
            let mut components: Vec<(f32, $crate::ComponentFn<_>)> = vec![$(($weight as f32, $comp(builder))),+];
            let mut temp_buffers = Vec::new();
            
            Box::new(move |runtime, input, output, sample_rate| {
                if temp_buffers.len() != components.len() {
                    temp_buffers.resize(components.len(), Vec::new());
                }
                for buf in &mut temp_buffers {
                    if buf.len() != output.len() {
                        buf.resize(output.len(), 0.0);
                    }
                }
                
                output.fill(0.0);
                for ((weight, comp), buf) in components.iter_mut().zip(temp_buffers.iter_mut()) {
                    buf.fill(0.0);
                    comp(runtime, input, buf, sample_rate);
                    
                    for (out, &sample) in output.iter_mut().zip(buf.iter()) {
                        *out += sample * *weight;
                    }
                }
            })
        }
    };
}

#[macro_export]
macro_rules! serial {
    ($($comp:expr),+) => {
        |builder: &mut $crate::Builder<_>| -> $crate::ComponentFn<_> {
            let mut components: Vec<$crate::ComponentFn<_>> = vec![$($comp(builder)),+];
            let mut buffer_a = Vec::new();
            let mut buffer_b = Vec::new();
            
            Box::new(move |runtime, input, output, sample_rate| {
                if components.is_empty() {
                    output.copy_from_slice(input);
                    return;
                }
                
                if buffer_a.len() != output.len() {
                    buffer_a.resize(output.len(), 0.0);
                    buffer_b.resize(output.len(), 0.0);
                }
                
                buffer_a.copy_from_slice(input);
                
                for (i, comp) in components.iter_mut().enumerate() {
                    let (inp, out) = if i % 2 == 0 {
                        (&buffer_a[..], &mut buffer_b[..])
                    } else {
                        (&buffer_b[..], &mut buffer_a[..])
                    };
                    out.fill(0.0);
                    comp(runtime, inp, out, sample_rate);
                }
                
                let final_buf = if components.len() % 2 == 1 { &buffer_b } else { &buffer_a };
                output.copy_from_slice(final_buf);
            })
        }
    };
}