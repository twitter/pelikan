// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Derive macros for `ccommon_rs`

extern crate proc_macro;

mod attrs;

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;
use syn::{
    parse_macro_input, Attribute, Data, DeriveInput, Error, Field, Fields, Generics, Ident, Lit,
    LitStr,
};

use self::attrs::*;

/// Derive macro for `Metrics`.
///
/// Fields in the struct attempting to use this derive macro
/// must either implement `SingleMetric` or `Metrics`. This is
/// decided upon based on the field being decorated with the
/// `metric` attribute.
///
/// # Example
/// ```rust,ignore
/// #[derive(Metrics)]
/// struct MyMetrics {
///     // This metric will have a name of `metric1` and a description
///     // of `"The first metric"`.
///     // This field requires that `Gauge` implements `SingleMetric`.
///     #[metric(desc = "The first metric")]
///     metric1: Gauge,
///     
///     // This metric will have a name of `mymetric.metric2` and a
///     // a description of `"The second metric"`.
///     // This field requires that `Counter` implements `SingleMetric`.
///     #[metric(desc = "The second metric", "mymetric.metric2")]
///     metric2: Counter
/// }
///
/// #[derive(Metrics)]
/// struct OtherMetrics {
///     // This field must implement `Metrics`
///     my_metrics: MyMetrics
/// }
/// ```
#[proc_macro_derive(Metrics, attributes(metric))]
pub fn derive_metrics(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(stream as DeriveInput);

    match derive_metrics_impl(input) {
        Ok(x) => x,
        Err(e) => e.to_compile_error(),
    }
    .into()
}

/// Derive macro for `Options`.
///
/// Fields in the struct attempting to use this derive macro
/// must either implement `SingleOption` or `Options`. Which
/// one of these will be required is decided by whether the
/// field is decorated by the `option` attribute.
///
/// The following parameters can be passed to the `option` attribute
/// - `desc`: Sets the description of the field.
/// - `name`: Overrides the name of the field, the default
///     name is the name of the field within the struct.
/// - `default`: Override the default value for the field.
///     By default, this is `Default::default()` or `std::ptr::null_mut()`
///     in the case of strings. This can be any valid rust expression
///     that returns the correct type.
///
/// # Example
/// ```rust,ignore
/// #[derive(Options)]
/// #[repr(C)]
/// struct MyOptions {
///     // This option will have a name of `options.option1`, a description
///     // of `Option 1`, and a default value of `false`.
///     #[option(desc = "Option 1", name = "options.option1")]
///     opt1: Bool,
///
///     #[option(
///         desc = "Function result option",
///         default = example_factory_function()
///     )]
///     opt2: UInt
/// }
///
/// #[derive(Option)]
/// #[repr(transparent)]
/// struct OtherOptions {
///     inner: MyOptions
/// }
/// ```
#[proc_macro_derive(Options, attributes(option))]
pub fn derive_options(stream: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let input = parse_macro_input!(stream as DeriveInput);

    match derive_options_impl(input) {
        Ok(x) => x,
        Err(e) => e.to_compile_error(),
    }
    .into()
}

fn derive_metrics_impl(input: DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let krate = crate_name("ccommon_rs")?;

    let data = match input.data {
        Data::Struct(data) => data,
        Data::Enum(data) => {
            return Err(Error::new(
                data.enum_token.span(),
                "Can only derive Metrics for a struct",
            ))
        }
        Data::Union(data) => {
            return Err(Error::new(
                data.union_token.span(),
                "Can only derive Metrics for a struct",
            ))
        }
    };

    if !is_repr_c_or_transparent(&input.attrs) {
        return Err(Error::new(
            input.ident.span(),
            format!(
                "`{}` must be either #[repr(C)] or #[repr(transparent)] to implement Metrics",
                input.ident
            ),
        ));
    }

    if has_generics(&input.generics) {
        return Err(Error::new(
            input.generics.span(),
            "Cannot derive Metrics for a struct with generics",
        ));
    }

    let ident = input.ident;
    let process_field = |is_tuple| {
        let krate = krate.clone();
        move |(i, field): (usize, &Field)| {
            let ty = &field.ty;
            let name = &field.ident;
            let label = if is_tuple {
                quote! {}
            } else {
                quote! { #name: }
            };

            Ok(match get_metric_attr(&field.attrs)? {
                Some(attr) => {
                    let desc = to_c_str(&attr.desc.val);
                    let namestr = match attr.name {
                        Some(name) => to_c_str(&name.val),
                        None => match field.ident.as_ref() {
                            Some(name) => to_c_str(&name_as_lit(name)),
                            None => {
                                to_c_str(&Lit::Str(LitStr::new(&format!("{}", i), field.span())))
                            }
                        },
                    };

                    quote! {
                        #label <#ty as #krate::metric::SingleMetric>::new(#namestr, #desc)
                    }
                }
                None => quote! {
                    #label <#ty as #krate::metric::Metrics>::new()
                },
            })
        }
    };

    let initializer = match data.fields {
        Fields::Named(fields) => {
            let initializers: Vec<_> = fields
                .named
                .iter()
                .enumerate()
                .map(process_field(false))
                .collect::<Result<_, Error>>()?;

            quote! {
                Self {
                    #( #initializers, )*
                }
            }
        }
        Fields::Unnamed(fields) => {
            let initializers: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .map(process_field(true))
                .collect::<Result<_, Error>>()?;

            quote! {
                Self (
                    #( #initializers, )*
                )
            }
        }
        Fields::Unit => quote!(Self),
    };

    Ok(quote! {
        unsafe impl #krate::metric::Metrics for #ident {
            fn new() -> Self {
                #initializer
            }
        }
    })
}

fn derive_options_impl(input: DeriveInput) -> Result<proc_macro2::TokenStream, Error> {
    let krate = crate_name("ccommon_rs")?;

    let data = match input.data {
        Data::Struct(data) => data,
        Data::Enum(data) => {
            return Err(Error::new(
                data.enum_token.span(),
                "Can only derive Metrics for a struct",
            ))
        }
        Data::Union(data) => {
            return Err(Error::new(
                data.union_token.span(),
                "Can only derive Metrics for a struct",
            ))
        }
    };

    if !is_repr_c_or_transparent(&input.attrs) {
        return Err(Error::new(
            input.ident.span(),
            format!(
                "`{}` must be either #[repr(C)] or #[repr(transparent)] to implement Metrics",
                input.ident
            ),
        ));
    }

    if has_generics(&input.generics) {
        return Err(Error::new(
            input.generics.span(),
            "Cannot derive Metrics for a struct with generics",
        ));
    }

    let ident = input.ident;
    let process_field = |is_tuple| {
        let krate = krate.clone();
        move |(i, field): (usize, &Field)| {
            let ty = &field.ty;
            let name = &field.ident;
            let label = if is_tuple {
                quote! {}
            } else {
                quote! { #name: }
            };

            Ok(match get_option_attr(&field.attrs)? {
                Some(attr) => {
                    let desc = to_c_str(&attr.desc.val);
                    let namestr = match attr.name {
                        Some(name) => to_c_str(&name.val),
                        None => match field.ident.as_ref() {
                            Some(name) => to_c_str(&name_as_lit(name)),
                            None => {
                                to_c_str(&Lit::Str(LitStr::new(&format!("{}", i), field.span())))
                            }
                        },
                    };

                    match attr.default.map(|x| x.val) {
                        Some(default) => quote! {
                            #label <#ty as #krate::option::SingleOption>::new(
                                #default,
                                #namestr,
                                #desc
                            )
                        },
                        None => quote! {
                            #label <#ty as #krate::option::SingleOption>::defaulted(#namestr, #desc)
                        },
                    }
                }
                None => quote! {
                    #label <#ty as #krate::option::Options>::new()
                },
            })
        }
    };

    let initializer = match data.fields {
        Fields::Named(fields) => {
            let initializers: Vec<_> = fields
                .named
                .iter()
                .enumerate()
                .map(process_field(false))
                .collect::<Result<_, Error>>()?;

            quote! {
                Self { #( #initializers, )* }
            }
        }
        Fields::Unnamed(fields) => {
            let initializers: Vec<_> = fields
                .unnamed
                .iter()
                .enumerate()
                .map(process_field(true))
                .collect::<Result<_, Error>>()?;

            quote! {
                Self ( #( #initializers, )* )
            }
        }
        Fields::Unit => quote!(Self),
    };

    Ok(quote! {
        unsafe impl #krate::option::Options for #ident {
            fn new() -> Self {
                #initializer
            }
        }
    })
}

fn crate_name(name: &'static str) -> Result<TokenStream, Error> {
    if std::env::var("CARGO_PKG_NAME").unwrap() == "ccommon_rs" {
        return Ok(quote! { ::ccommon_rs });
    }

    let name = match proc_macro_crate::crate_name(name) {
        Ok(name) => name,
        Err(e) => return Err(Error::new(Span::call_site(), e)),
    };

    let ident = Ident::new(&name, Span::call_site());

    Ok(quote! { ::#ident })
}

fn name_as_lit(name: &Ident) -> Lit {
    Lit::Str(LitStr::new(&format!("{}", name), name.span()))
}

fn to_c_str(desc: &Lit) -> TokenStream {
    use syn::LitByteStr;

    let span = desc.span();

    let mut value = match desc {
        Lit::Str(lit) => lit.value().into_bytes(),
        Lit::ByteStr(lit) => lit.value(),
        _ => unreachable!(),
    };

    for &byte in &value {
        if byte == b'\0' {
            return quote_spanned! { span =>
                compile_error!(
                    "Description contained a nul character"
                )
            };
        }
    }

    value.push(b'\0');

    let lit = LitByteStr::new(&value, span);

    quote! {
        unsafe {
            ::std::ffi::CStr::from_bytes_with_nul_unchecked(#lit)
        }
    }
}

fn get_metric_attr(attrs: &[Attribute]) -> Result<Option<MetricAttr>, Error> {
    for attr in attrs {
        let ident = match attr.path.get_ident() {
            Some(x) => x,
            None => continue,
        };

        if ident != "metric" {
            continue;
        }

        return Ok(Some(attr.parse_args()?));
    }

    Ok(None)
}

fn get_option_attr(attrs: &[Attribute]) -> Result<Option<OptionAttr>, Error> {
    for attr in attrs {
        let ident = match attr.path.get_ident() {
            Some(x) => x,
            None => continue,
        };

        if ident != "option" {
            continue;
        }

        return Ok(Some(attr.parse_args()?));
    }

    Ok(None)
}

fn is_repr_c_or_transparent(attrs: &[Attribute]) -> bool {
    use syn::{Meta, NestedMeta};

    fn is_correct_meta(nested: &NestedMeta) -> bool {
        let meta = match nested {
            NestedMeta::Meta(meta) => meta,
            _ => return false,
        };

        let path = match meta {
            Meta::Path(path) => path,
            _ => return false,
        };

        let ident = match path.get_ident() {
            Some(ident) => ident,
            _ => return false,
        };

        ident == "C" || ident == "transparent"
    }

    for attr in attrs {
        let ident = match attr.path.get_ident() {
            Some(ident) => ident,
            None => continue,
        };

        if ident != "repr" {
            continue;
        }

        let list = match attr.parse_meta() {
            Ok(Meta::List(list)) => list,
            _ => continue,
        };

        for nested in &list.nested {
            if is_correct_meta(nested) {
                return true;
            }
        }
    }

    false
}

fn has_generics(generics: &Generics) -> bool {
    generics.lt_token.is_some() || generics.where_clause.is_some()
}
