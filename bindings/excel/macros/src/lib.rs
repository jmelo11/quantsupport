//! Ownership scopes begin before any worksheet argument conversion can fail.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, FnArg, ItemFn, Pat, ReturnType, Type};

/// Keeps xllgen's registration and result conversion while owning the full call.
#[proc_macro_attribute]
pub fn xll_bindgen(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = proc_macro2::TokenStream::from(args);
    let function = parse_macro_input!(input as ItemFn);
    let threadsafe = args
        .clone()
        .into_iter()
        .any(|token| token.to_string() == "threadsafe");
    let is_result = matches!(&function.sig.output,
        ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Path(path)
            if path.path.segments.last().is_some_and(|part| part.ident == "Result")));
    if threadsafe || !is_result {
        return quote!(#[xllgen::xll_bindgen(#args)] #function).into();
    }
    let name = &function.sig.ident;
    let module = format_ident!("__{}_worksheet", name);
    let export = format_ident!("xl_{}", name);
    let mut signature = function.sig.clone();
    let mut conversions = Vec::new();
    let mut call_args = Vec::new();
    for argument in &mut signature.inputs {
        let FnArg::Typed(argument) = argument else {
            continue;
        };
        let Pat::Ident(binding) = argument.pat.as_ref() else {
            return syn::Error::new_spanned(argument, "worksheet arguments must be named")
                .to_compile_error()
                .into();
        };
        let name = &binding.ident;
        let ty = &argument.ty;
        if matches!(ty.as_ref(), Type::Ptr(_)) {
            call_args.push(quote!(#name));
        } else if matches!(ty.as_ref(), Type::Reference(_)) {
            conversions
                .push(quote!(let #name: String = crate::runtime::arguments::decode(#name)?;));
            call_args.push(quote!(#name.as_str()));
        } else {
            conversions.push(quote!(let #name: #ty = crate::runtime::arguments::decode(#name)?;));
            call_args.push(quote!(#name));
        }
        argument.ty = syn::parse_quote!(*const xll_rs::types::XLOPER12);
    }
    quote! {
        #function
        #[allow(unused_imports)]
        mod #module {
            use super::*;
            #[xllgen::xll_bindgen(#args)]
            #[allow(clippy::too_many_arguments)]
            pub #signature {
                let scope = crate::runtime::lifecycle::CallScope::enter();
                #[cfg(not(test))]
                if !scope.has_caller() {
                    return Err(crate::functions::helpers::value_error("call this function from a worksheet cell so its objects have a workbook owner"));
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #(#conversions)*
                    super::#name(#(#call_args),*)
                })).unwrap_or_else(|_| Err(crate::functions::helpers::value_error("native evaluation panicked; previous cell state was released")));
                scope.finish(result.is_ok());
                result
            }
        }
        #[allow(unused_imports)]
        pub use #module::#export;
    }.into()
}
