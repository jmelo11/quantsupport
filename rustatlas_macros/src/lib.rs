use proc_macro::TokenStream;
use quote::quote;
use syn::{
    Attribute, Block, Expr, ExprLit, Fields, FnArg, Ident, Item, ItemFn, ItemStruct, Lit,
    ReturnType, fold::Fold, parse_macro_input,
};

/// # differentiable
/// Transforms methods and struct into differentiable counterparts.
#[proc_macro_attribute]
pub fn differentiable(attr: TokenStream, item: TokenStream) -> TokenStream {
    let diff_name = parse_macro_input!(attr as Ident);
    let input = parse_macro_input!(item as Item);

    match input {
        Item::Struct(item_struct) => expand_struct(diff_name, item_struct),
        Item::Fn(item_fn) => expand_fn(diff_name, item_fn),
        other => syn::Error::new_spanned(
            other,
            "differentiable can only be applied to structs or functions",
        )
        .to_compile_error()
        .into(),
    }
}

fn expand_struct(diff_name: Ident, item_struct: ItemStruct) -> TokenStream {
    let struct_data = match item_struct.fields {
        Fields::Named(fields_named) => fields_named,
        _ => {
            return syn::Error::new_spanned(
                item_struct.ident,
                "differentiable only supports structs with named fields",
            )
            .to_compile_error()
            .into();
        }
    };

    let vis = item_struct.vis;
    let original_name = item_struct.ident;
    let diff_struct_name = diff_name;

    let mut original_fields = Vec::new();
    let mut diff_fields = Vec::new();

    for field in struct_data.named {
        let is_diffvar = field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("diffvar"));

        let filtered_attrs: Vec<Attribute> = field
            .attrs
            .clone()
            .into_iter()
            .filter(|attr| !attr.path().is_ident("diffvar"))
            .collect();

        let mut original_field = field.clone();
        original_field.attrs = filtered_attrs.clone();

        let mut diff_field = field;
        diff_field.attrs = filtered_attrs;

        if is_diffvar {
            diff_field.ty = syn::parse_quote!(::rustatlas::ad::adreal::ADReal);
        }

        original_fields.push(original_field);
        diff_fields.push(diff_field);
    }

    let expanded = quote! {
        #vis struct #original_name {
            #(#original_fields,)*
        }

        #vis struct #diff_struct_name {
            #(#diff_fields,)*
        }

        impl ::rustatlas::ad::Differentiable for #diff_struct_name {}
    };

    expanded.into()
}

fn expand_fn(diff_name: Ident, item_fn: ItemFn) -> TokenStream {
    let vis = item_fn.vis;
    let original_name = item_fn.sig.ident.clone();

    let original_return_type = match &item_fn.sig.output {
        ReturnType::Type(_, ty) if is_f64(ty) => item_fn.sig.output.clone(),
        _ => {
            return syn::Error::new_spanned(
                item_fn.sig.output,
                "differentiable functions must return f64",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut original_inputs = item_fn.sig.inputs.clone();
    let mut diff_inputs = item_fn.sig.inputs.clone();
    let mut diff_params = Vec::new();

    for (original_arg, diff_arg) in original_inputs.iter_mut().zip(diff_inputs.iter_mut()) {
        match (original_arg, diff_arg) {
            (FnArg::Typed(original_pat), FnArg::Typed(diff_pat)) => {
                let is_diffvar = original_pat
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("diffvar"));

                let filtered_attrs: Vec<Attribute> = original_pat
                    .attrs
                    .iter()
                    .cloned()
                    .filter(|attr| !attr.path().is_ident("diffvar"))
                    .collect();

                original_pat.attrs = filtered_attrs.clone();
                diff_pat.attrs = filtered_attrs;

                if is_diffvar {
                    diff_pat.ty = Box::new(syn::parse_quote!(::rustatlas::ad::adreal::ADReal));
                    diff_params.push(DiffParam { const_shadow: None });
                } else {
                    let const_shadow = match &*original_pat.pat {
                        syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.clone()),
                        _ => None,
                    };
                    diff_params.push(DiffParam { const_shadow });
                }
            }
            _ => {}
        }
    }

    let mut original_sig = item_fn.sig.clone();
    original_sig.ident = original_name;
    original_sig.inputs = original_inputs;
    original_sig.output = original_return_type;

    let mut diff_sig = item_fn.sig.clone();
    diff_sig.ident = diff_name;
    diff_sig.inputs = diff_inputs;
    diff_sig.output = syn::parse_quote!(-> ::rustatlas::ad::adreal::ADReal);

    let original_block = item_fn.block;
    let diff_block: Block = {
        let const_params: Vec<Ident> = diff_params
            .iter()
            .filter_map(|param| param.const_shadow.clone())
            .collect();
        let mut block = *original_block.clone();
        let mut literal_folder = LiteralToConst;
        block = literal_folder.fold_block(block);
        let inner_block = Block {
            brace_token: block.brace_token,
            stmts: block.stmts,
        };
        syn::parse_quote!({
            #(
                let #const_params = ::rustatlas::ad::adreal::Const::from(#const_params);
            )*
            (|| #inner_block)().into()
        })
    };

    let expanded = quote! {
        #vis #original_sig #original_block

        #vis #diff_sig #diff_block
    };

    expanded.into()
}

fn is_f64(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(path) => path
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == "f64"),
        _ => false,
    }
}

#[derive(Clone)]
struct DiffParam {
    const_shadow: Option<Ident>,
}

struct LiteralToConst;

impl Fold for LiteralToConst {
    fn fold_expr(&mut self, expr: Expr) -> Expr {
        match expr {
            Expr::Lit(ExprLit {
                lit: Lit::Float(lit),
                ..
            }) => syn::parse_quote!(::rustatlas::ad::adreal::Const::from(#lit)),
            Expr::Lit(ExprLit {
                lit: Lit::Int(lit), ..
            }) => syn::parse_quote!(::rustatlas::ad::adreal::Const::from(#lit)),
            other => syn::fold::fold_expr(self, other),
        }
    }
}
