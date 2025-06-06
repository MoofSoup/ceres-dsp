//ceres-dsp/ceres-macros/src/lib.rs
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Data, Fields};
use syn::spanned::Spanned;

#[proc_macro_attribute]
pub fn parameters(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let struct_name = &input.ident;
    let runtime_name = syn::Ident::new(&format!("{}Runtime", struct_name), struct_name.span());
    let accessor_name = syn::Ident::new(&format!("{}Accessor", struct_name), struct_name.span());
    
    let fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(fields) => &fields.named,
            _ => return syn::Error::new(struct_name.span(), "Only named fields supported")
                .to_compile_error().into(),
        },
        _ => return syn::Error::new(struct_name.span(), "Only structs supported")
            .to_compile_error().into(),
    };
    
    // Validate f32 fields
    for field in fields.iter() {
        let field_name = field.ident.as_ref().unwrap();
        if let syn::Type::Path(type_path) = &field.ty {
            if let Some(segment) = type_path.path.segments.last() {
                if segment.ident != "f32" {
                    return syn::Error::new(
                        field.span(), 
                        format!("Parameter field '{}' must be f32", field_name)
                    ).to_compile_error().into();
                }
            }
        } else {
            return syn::Error::new(
                field.span(), 
                format!("Parameter field '{}' must be f32", field_name)
            ).to_compile_error().into();
        }
    }
    
    let field_names: Vec<_> = fields.iter().map(|f| &f.ident).collect();
    
    // Generate modulation field names
    let mod_field_names: Vec<_> = field_names.iter().map(|name| {
        syn::Ident::new(&format!("{}_modulation", name.as_ref().unwrap()), name.span())
    }).collect();
    
    let mod_fields = mod_field_names.iter().map(|mod_name| {
        quote! { #mod_name: Option<::ceres::ModulationRouting> }
    });
    
    // Generate route methods
    let route_methods = field_names.iter().zip(mod_field_names.iter()).map(|(name, mod_name)| {
        let method_name = syn::Ident::new(&format!("route_{}", name.as_ref().unwrap()), name.span());
        quote! {
            fn #method_name(&mut self, source_index: usize, amount: f32) {
                self.#mod_name = Some(::ceres::ModulationRouting { source_index, amount });
            }
        }
    });
    
    // Generate route_parameter match arms
    let route_arms = field_names.iter().zip(mod_field_names.iter()).map(|(name, _)| {
        let name_str = name.as_ref().unwrap().to_string();
        let method_name = syn::Ident::new(&format!("route_{}", name.as_ref().unwrap()), name.span());
        quote! { #name_str => self.#method_name(source_index, amount) }
    });
    
    // Generate update logic
    let update_fields = field_names.iter().zip(mod_field_names.iter()).map(|(name, mod_name)| {
        quote! {
            let #name = self.#mod_name
                .as_ref()
                .map(|routing| {
                    let modulator_value = sources[routing.source_index].get_value(i);
                    modulator_value * routing.amount
                })
                .unwrap_or(0.0);
            let #name = (self.base.#name + #name).clamp(0.0, 1.0);
        }
    });
    
    let expanded = quote! {
        #[derive(Clone, Copy, Default)]
        #input
        
        struct #runtime_name<E> {
            base: #struct_name,
            #(#mod_fields,)*
            computed_values: [#struct_name; ::ceres::BUFFER_SIZE],
        }
        
        impl<E> #runtime_name<E> {
            fn new() -> Self {
                let base = #struct_name::default();
                Self {
                    base,
                    #(#mod_field_names: None,)*
                    computed_values: [base; ::ceres::BUFFER_SIZE],
                }
            }
            
            #(#route_methods)*
        }
        
        impl<E: Send + 'static> ::ceres::ParameterRuntime<E> for #runtime_name<E> {
            fn update(&mut self, sources: &[Box<dyn ::ceres::Modulator<E>>]) {
                for i in 0..::ceres::BUFFER_SIZE {
                    #(#update_fields)*
                    self.computed_values[i] = #struct_name {
                        #(#field_names: #field_names),*
                    };
                }
            }
            
            fn route_parameter(&mut self, param_name: &str, source_index: usize, amount: f32) {
                match param_name {
                    #(#route_arms,)*
                    _ => {}
                }
            }
        }
        
        struct #accessor_name<'a> {
            values: &'a [#struct_name; ::ceres::BUFFER_SIZE],
        }
        
        impl<'a> #accessor_name<'a> {
            fn new(values: &'a [#struct_name; ::ceres::BUFFER_SIZE]) -> Self {
                Self { values }
            }
        }
        
        impl<'a> std::ops::Index<usize> for #accessor_name<'a> {
            type Output = #struct_name;
            fn index(&self, index: usize) -> &Self::Output {
                &self.values[index % ::ceres::BUFFER_SIZE]
            }
        }
        
        impl ::ceres::Parameters for #struct_name {
            type Runtime<E: Send + 'static> = #runtime_name<E>;
            type Accessor<'a, E> = #accessor_name<'a> where E: 'a;
            type Values = #struct_name;
            
            fn create_runtime<E: Send>() -> Self::Runtime<E> {
                #runtime_name::new()
            }
            
            fn create_accessor<E: Send>(runtime: &Self::Runtime<E>) -> Self::Accessor<'_, E> {
                #accessor_name::new(&runtime.computed_values)
            }
        }
    };
    
    TokenStream::from(expanded)
}