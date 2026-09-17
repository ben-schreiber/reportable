//! Derive implementation for `reportable`. Use the re-export from that crate.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Fields, Ident, parse_macro_input, parse_quote};

/// Derive error classification from `#[reportable(caller | internal | transparent)]`.
#[proc_macro_derive(Reportable, attributes(reportable))]
pub fn derive_reportable(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

fn expand(input: DeriveInput) -> syn::Result<TokenStream2> {
    let path = match crate_name("reportable") {
        Ok(FoundCrate::Itself) => quote!(::reportable),
        Ok(FoundCrate::Name(name)) => {
            let name = Ident::new(&name, input.ident.span());
            quote!(::#name)
        }
        Err(error) => return Err(Error::new_spanned(&input.ident, error)),
    };
    expand_with_path(input, &path)
}

fn expand_with_path(input: DeriveInput, path: &TokenStream2) -> syn::Result<TokenStream2> {
    reject_misplaced(&input.attrs)?;
    let Data::Enum(data) = input.data else {
        return Err(Error::new_spanned(
            input.ident,
            "Reportable only supports enums",
        ));
    };

    let mut generics = input.generics;
    let mut arms = Vec::new();
    for variant in data.variants {
        for field in &variant.fields {
            reject_misplaced(&field.attrs)?;
        }
        let mode = classification(&variant.attrs, &variant.ident)?;
        let name = &variant.ident;
        let arm = match mode {
            Classification::Caller | Classification::Internal => {
                let destination = match mode {
                    Classification::Caller => quote!(#path::ReportTo::Caller),
                    _ => quote!(#path::ReportTo::Internal),
                };
                let pattern = match variant.fields {
                    Fields::Unit => quote!(Self::#name),
                    Fields::Unnamed(_) => quote!(Self::#name(..)),
                    Fields::Named(_) => quote!(Self::#name { .. }),
                };
                quote!(#pattern => #destination)
            }
            Classification::Transparent => {
                if variant.fields.len() != 1 {
                    return Err(Error::new_spanned(
                        &variant,
                        "#[reportable(transparent)] requires exactly one field",
                    ));
                }
                let field = &variant.fields.iter().next().ok_or_else(|| {
                    Error::new_spanned(&variant, "transparent variant has no field")
                })?;
                let ty = &field.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: #path::Reportable));

                let pattern = match &field.ident {
                    Some(field_name) => quote!(Self::#name { #field_name: inner }),
                    None => quote!(Self::#name(inner)),
                };
                quote!(#pattern => #path::Reportable::report_to(inner))
            }
        };
        arms.push(arm);
    }

    let name = input.ident;
    let (impl_generics, type_generics, where_clause) = generics.split_for_impl();
    // Dereferencing also permits an exhaustive match on an uninhabited enum.
    let body = if arms.is_empty() {
        quote!(match *self {})
    } else {
        quote!(match self { #(#arms,)* })
    };
    Ok(quote! {
        impl #impl_generics #path::Reportable for #name #type_generics #where_clause {
            fn report_to(&self) -> #path::ReportTo {
                #body
            }
        }
    })
}

fn reject_misplaced(attrs: &[Attribute]) -> syn::Result<()> {
    if let Some(attr) = attrs.iter().find(|attr| attr.path().is_ident("reportable")) {
        return Err(Error::new_spanned(
            attr,
            "#[reportable(...)] belongs on an enum variant",
        ));
    }
    Ok(())
}

enum Classification {
    Caller,
    Internal,
    Transparent,
}

fn classification(attrs: &[Attribute], name: &Ident) -> syn::Result<Classification> {
    let mut attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("reportable"));
    let attr = attrs.next().ok_or_else(|| {
        Error::new_spanned(
            name,
            "add #[reportable(caller)], #[reportable(internal)], or #[reportable(transparent)]",
        )
    })?;
    if let Some(duplicate) = attrs.next() {
        return Err(Error::new_spanned(
            duplicate,
            "use exactly one #[reportable(...)] annotation per variant",
        ));
    }
    let mode: Ident = attr.parse_args()?;
    match mode.to_string().as_str() {
        "caller" => Ok(Classification::Caller),
        "internal" => Ok(Classification::Internal),
        "transparent" => Ok(Classification::Transparent),
        _ => Err(Error::new_spanned(
            mode,
            "expected caller, internal, or transparent",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_ambiguous_or_misplaced_classification() {
        let cases = [
            (
                quote!(
                    enum E {
                        Missing,
                    }
                ),
                "add #[reportable(caller)]",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(unknown)]
                        V,
                    }
                ),
                "expected caller",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(caller)]
                        #[reportable(internal)]
                        V,
                    }
                ),
                "exactly one",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(transparent)]
                        V,
                    }
                ),
                "exactly one field",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(transparent)]
                        V(u8, u8),
                    }
                ),
                "exactly one field",
            ),
            (
                quote!(
                    #[reportable(caller)]
                    enum E {
                        V,
                    }
                ),
                "belongs on an enum variant",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(internal)]
                        V(#[reportable(caller)] u8),
                    }
                ),
                "belongs on an enum variant",
            ),
            (
                quote!(
                    struct E;
                ),
                "only supports enums",
            ),
            (quote!(union E { value: u8 }), "only supports enums"),
        ];
        for (input, expected) in cases {
            let input = syn::parse2(input).unwrap();
            let error = expand_with_path(input, &quote!(::reportable)).unwrap_err();
            assert!(error.to_string().contains(expected), "{error}");
        }

        for input in [
            quote!(
                enum E {
                    #[reportable()]
                    V,
                }
            ),
            quote!(
                enum E {
                    #[reportable(caller, internal)]
                    V,
                }
            ),
            quote!(
                enum E {
                    #[reportable = "caller"]
                    V,
                }
            ),
        ] {
            assert!(expand_with_path(syn::parse2(input).unwrap(), &quote!(::reportable)).is_err());
        }
    }
}
