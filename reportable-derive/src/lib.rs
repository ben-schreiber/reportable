//! Derive implementation for `reportable`. Use the re-export from that crate.

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Fields, Ident, parse_macro_input, parse_quote};

/// Derive error classification from `#[reportable(caller | internal | transparent)]`.
///
/// On a struct, the attribute classifies the whole type. On an enum, it sets a
/// default for every variant; a variant-level attribute overrides that default.
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
    let default = classification(&input.attrs, "type")?;
    // A struct is classified like a single variant matched by `Self`.
    let cases = match input.data {
        Data::Struct(data) => vec![(quote!(Self), input.ident.clone(), data.fields, None)],
        Data::Enum(data) => data
            .variants
            .into_iter()
            .map(|variant| {
                let name = &variant.ident;
                let mode = classification(&variant.attrs, "variant")?;
                Ok((quote!(Self::#name), variant.ident, variant.fields, mode))
            })
            .collect::<syn::Result<_>>()?,
        Data::Union(_) => {
            return Err(Error::new_spanned(
                input.ident,
                "Reportable only supports structs and enums",
            ));
        }
    };

    let mut generics = input.generics;
    let mut arms = Vec::new();
    for (prefix, ident, fields, mode) in cases {
        for field in &fields {
            reject_misplaced(&field.attrs)?;
        }
        let mode = mode.or(default).ok_or_else(|| {
            Error::new_spanned(
                &ident,
                "add #[reportable(caller)], #[reportable(internal)], or #[reportable(transparent)]",
            )
        })?;
        let arm = match mode {
            Classification::Caller | Classification::Internal => {
                let destination = match mode {
                    Classification::Caller => quote!(#path::ReportTo::Caller),
                    _ => quote!(#path::ReportTo::Internal),
                };
                let pattern = match fields {
                    Fields::Unit => quote!(#prefix),
                    Fields::Unnamed(_) => quote!(#prefix(..)),
                    Fields::Named(_) => quote!(#prefix { .. }),
                };
                quote!(#pattern => #destination)
            }
            Classification::Transparent => {
                let mut iter = fields.iter();
                let (Some(field), None) = (iter.next(), iter.next()) else {
                    return Err(Error::new_spanned(
                        &ident,
                        "#[reportable(transparent)] requires exactly one field",
                    ));
                };
                let ty = &field.ty;
                generics
                    .make_where_clause()
                    .predicates
                    .push(parse_quote!(#ty: #path::Reportable));

                let pattern = match &field.ident {
                    Some(field_name) => quote!(#prefix { #field_name: inner }),
                    None => quote!(#prefix(inner)),
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
            "#[reportable(...)] belongs on a struct, enum, or enum variant",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Classification {
    Caller,
    Internal,
    Transparent,
}

fn classification(attrs: &[Attribute], scope: &str) -> syn::Result<Option<Classification>> {
    let mut attrs = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("reportable"));
    let Some(attr) = attrs.next() else {
        return Ok(None);
    };
    if let Some(duplicate) = attrs.next() {
        return Err(Error::new_spanned(
            duplicate,
            format!("use exactly one #[reportable(...)] annotation per {scope}"),
        ));
    }
    let mode: Ident = attr.parse_args()?;
    match mode.to_string().as_str() {
        "caller" => Ok(Some(Classification::Caller)),
        "internal" => Ok(Some(Classification::Internal)),
        "transparent" => Ok(Some(Classification::Transparent)),
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
                    #[reportable(internal)]
                    enum E {
                        V,
                    }
                ),
                "exactly one",
            ),
            (
                quote!(
                    #[reportable(transparent)]
                    enum E {
                        V,
                    }
                ),
                "exactly one field",
            ),
            (
                quote!(
                    enum E {
                        #[reportable(internal)]
                        V(#[reportable(caller)] u8),
                    }
                ),
                "belongs on a struct, enum, or enum variant",
            ),
            (
                quote!(
                    struct E;
                ),
                "add #[reportable(caller)]",
            ),
            (
                quote!(
                    #[reportable(transparent)]
                    struct E(u8, u8);
                ),
                "exactly one field",
            ),
            (
                quote!(
                    #[reportable(internal)]
                    struct E(#[reportable(caller)] u8);
                ),
                "belongs on a struct",
            ),
            (
                quote!(union E { value: u8 }),
                "only supports structs and enums",
            ),
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
