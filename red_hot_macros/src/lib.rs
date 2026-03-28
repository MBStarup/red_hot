use proc_macro::TokenStream;
use quote::quote;
use syn::{Data, Fields};

#[proc_macro_derive(Vertex)]
//. Derives Vertex for a given struct with named fields.
//. Struct must be #[repr(C)]
//. All fields must be named, and implement GlslType
// TODO: In the future, consider allowing Vertex to be constructed from other Vertex's. Allowing for a more flexible trait, and reomving the need for the separate GlslType trait, at teh cost of more complex and involved derive, and potentially the need to do runtime calculations of the attribute array (not the biggest deal since it's not a hot path)
pub fn derive_vertex(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();
    let name = ast.ident;

    fn has_repr_c(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|attr| {
            if !attr.path().is_ident("repr") {
                return false;
            }

            //. Parses the args of the arrtibute as a comma seperated list, looking for any which is just "C"
            match attr.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated) {
                Ok(args) => args.iter().any(|p| p.is_ident("C")),
                Err(_) => false,
            }
        })
    }

    if !has_repr_c(&ast.attrs) {
        return syn::Error::new_spanned(&name, "Vertex structs must have #[repr(C)]").to_compile_error().into();
    }

    let fields = match ast.data {
        Data::Struct(s) => match s.fields {
            Fields::Named(fields) => fields.named,
            _ => return syn::Error::new_spanned(name, "Vertex can only be derived for structs with named fields").to_compile_error().into(),
        },
        _ => return syn::Error::new_spanned(name, "Vertex can only be derived for structs").to_compile_error().into(),
    };

    let field_count = fields.len();

    let attributes = fields.iter().enumerate().map(|(i, field)| {
        let location = i as u32;
        let ty = &field.ty;
        let field_name = &field.ident;

        //. I think offset_of is (probably) fine
        //. As far as I understand
        let attribute = quote! {
            ::ash::vk::VertexInputAttributeDescription {
                location: #location,
                binding: 0, // TODO: Figure out the actual use case for different bindings. If there is one, take binding as a parameter or something
                format: <#ty as ::red_hot::GlslType>::TYPE,
                offset: ::std::mem::offset_of!(#name, #field_name) as u32,
            }
        };

        return attribute;
    });

    let expanded = quote! {
        impl ::red_hot::Vertex for #name {
            type AttributeDescriptions = [::ash::vk::VertexInputAttributeDescription; #field_count];

            const ATTRIBUTE_DESCRIPTIONS: Self::AttributeDescriptions = [
                #(#attributes),*
            ];
        }
    };

    TokenStream::from(expanded)
}
