//! Ownership scopes begin before any worksheet argument conversion can fail.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
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
    let attributes = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
        .parse2(args.clone())
        .expect("valid worksheet attributes");
    let worksheet_name = attributes
        .iter()
        .find_map(|attribute| match attribute {
            syn::Meta::NameValue(value) if value.path.is_ident("name") => Some(value.value.clone()),
            _ => None,
        })
        .expect("worksheet functions have a registered name");
    let module = format_ident!("__{}_worksheet", name);
    let export = format_ident!("xl_{}", name);
    let asynchronous = name.to_string().starts_with("qs_instrument_")
        || name == "qs_trade_create"
        || name.to_string().starts_with("qs_evaluate")
        || name == "qs_portfolio_evaluate"
        || name.to_string().starts_with("qs_result_")
        || name == "qs_portfolio_result_cashflows"
        || name.to_string().starts_with("qs_curve_")
        || name.to_string().starts_with("qs_simulation_")
        || name.to_string().starts_with("qs_vol_")
        || name == "qs_context_quote"
        || name == "qs_context_fixing"
        || name == "qs_context_curves"
        || name == "qs_context_vol_surfaces"
        || name == "qs_context_vol_cubes"
        || name == "qs_context_simulations"
        || name == "qs_context_from_snapshot"
        || name.to_string().starts_with("qs_snapshots_")
        || name.to_string().starts_with("qs_snapshot_")
        || name == "qs_context_evaluation_date"
        || name == "qs_context_snapshot_timestamp"
        || name == "qs_context_quote_level";
    let mut signature = function.sig.clone();
    let mut conversions = Vec::new();
    let mut call_args = Vec::new();
    let mut raw_args = Vec::new();
    let mut replay_args = Vec::new();
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
        let position = raw_args.len();
        raw_args.push(quote!(#name));
        replay_args
            .push(quote!(let #name = operands.0[#position] as *const xll_rs::types::XLOPER12;));
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
    if asynchronous {
        signature.output =
            syn::parse_quote!(-> Result<*mut xll_rs::types::XLOPER12, xll_rs::types::XllError>);
    }
    let execution = if asynchronous {
        quote!(crate::runtime::engine::bridge::worksheet(
            stringify!(#name),
            &__raw_args
        ))
    } else {
        quote!({
            #(#conversions)*
            let value = super::#name(#(#call_args),*)?;
            crate::runtime::engine::bridge::record(stringify!(#name), &__raw_args)?;
            Ok(value)
        })
    };
    quote! {
        #function
        #[allow(unused_imports)]
        mod #module {
            use super::*;
            fn replay(arguments: &[crate::runtime::engine::bridge::Value]) -> Result<crate::runtime::engine::bridge::Value, String> {
                let operands = crate::runtime::engine::bridge::Operands::new(arguments);
                #(#replay_args)*
                let result: Result<_, xll_rs::types::XllError> = (|| {
                    #(#conversions)*
                    super::#name(#(#call_args),*)
                })();
                crate::runtime::engine::bridge::Detach::detach(result.map_err(|error| error.message().unwrap_or("invalid engine inputs").to_string())?)
            }
            xll_rs::inventory::submit! { crate::runtime::engine::bridge::Replay { name: stringify!(#name), invoke: replay } }
            #[xllgen::xll_bindgen(#args)]
            #[allow(clippy::too_many_arguments)]
            pub #signature {
                let __raw_args = [#(#raw_args as *const xll_rs::types::XLOPER12),*];
                let __began = std::time::Instant::now();
                let scope = crate::runtime::lifecycle::CallScope::enter(stringify!(#name), #worksheet_name);
                let __caller_ms = __began.elapsed().as_secs_f64() * 1000.;
                #[cfg(not(test))]
                if !scope.has_caller() {
                    return Err(crate::functions::helpers::value_error("call this function from a worksheet cell so its objects have a workbook owner"));
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    #execution
                })).unwrap_or_else(|_| Err(crate::functions::helpers::value_error("native evaluation panicked; previous cell state was released")));
                if !#asynchronous || result.is_err() {
                    crate::runtime::registry::with_registry_mut(|registry|
                        registry.record_diagnostic(result.as_ref().err().map(|error| error.message().unwrap_or("The input contains an Excel error or has the wrong type.").to_string())
                            .or_else(|| registry.engine_cell().and_then(crate::runtime::engine::bridge::source_error))));
                }
                if let Err(error) = &result {
                    if let Some(cell) = crate::runtime::registry::with_registry(|r| r.engine_cell()) {
                        crate::runtime::engine::bridge::failed(cell, error.message().unwrap_or("invalid worksheet inputs").to_string());
                    }
                }
                scope.finish(result.is_ok());
                crate::runtime::engine::bridge::record_call(__caller_ms, __began.elapsed().as_secs_f64() * 1000.);
                // xllgen renders an XllError message as worksheet text. Keep
                // diagnostics in the source registry and return a typed error.
                result.map_err(|error| xll_rs::types::XllError::from(error.code()))
            }
        }
        #[allow(unused_imports)]
        pub use #module::#export;
    }.into()
}
