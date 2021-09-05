use proc_macro::TokenStream;
use quote::quote;
use syn::Ident;

/// This macro statically converts an ident to a lowercased string
/// at compile time.
///
/// In the future this could be replaced with some const code. However,
/// `std::str::from_utf8_unchecked` is not stably const just yet so we
/// need this macro as a workaround.
#[proc_macro]
pub fn to_lowercase(input: TokenStream) -> TokenStream {
    let ident = syn::parse_macro_input!(input as Ident);
    let name = ident.to_string().to_ascii_lowercase();
    let literal = syn::LitStr::new(&name, ident.span());
    let tokens = quote! { #literal };

    tokens.into()
}
