//! `#[derive(ConfigUi)]`: implements `common::menu_schema::ConfigUi` for a
//! config struct - one `UiField` per field, so the in-game menu can draw
//! the struct without a separate schema file (2026-10-02; the struct is the
//! single description of the config: serde reads/writes it, this describes
//! it to the menu, the template holds the defaults).
//!
//! Per field:
//! - key: the name serde uses - `#[serde(rename = "...")]`, else the
//!   container's `#[serde(rename_all = "PascalCase")]` applied to the field
//!   name (only `PascalCase` and no rename are supported).
//! - label: `#[ui(label = "...")]`, else the key.
//! - description: the field's `///` doc comment.
//! - kind, from the Rust type: `f32`/`f64` -> Float, integer types -> Int,
//!   `bool` -> Bool, `String` -> Text (`#[ui(key)]` -> Key, a hotkey
//!   picker), `Vec<_>` -> List, `Option<_>` -> skipped, any other type ->
//!   Group (that type's own `ConfigUi::ui_fields()`).
//! - `#[ui(hidden)]` leaves the field out.
//!
//! Float/Int ranges: `#[ui(min = .., max = .., step = ..)]` on the field,
//! else the same attribute on the struct (a default for all its numbers),
//! else 0..10 step 0.01 (floats) / 0..100 (ints).

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Expr, ExprLit, Fields, Lit, Meta, Type, parse_macro_input};

#[proc_macro_derive(ConfigUi, attributes(ui))]
pub fn derive_config_ui(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    match expand(&input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

#[derive(Default, Clone)]
struct UiAttrs {
    label: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    step: Option<f64>,
    key: bool,
    hidden: bool,
}

fn number(expr: &Expr) -> syn::Result<f64> {
    let (negative, inner) = match expr {
        Expr::Unary(u) if matches!(u.op, syn::UnOp::Neg(_)) => (true, &*u.expr),
        other => (false, other),
    };
    let value = match inner {
        Expr::Lit(ExprLit { lit: Lit::Float(f), .. }) => f.base10_parse::<f64>()?,
        Expr::Lit(ExprLit { lit: Lit::Int(i), .. }) => i.base10_parse::<i64>()? as f64,
        _ => return Err(syn::Error::new_spanned(expr, "expected a number")),
    };
    Ok(if negative { -value } else { value })
}

fn ui_attrs(attrs: &[Attribute]) -> syn::Result<UiAttrs> {
    let mut out = UiAttrs::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("ui")) {
        attr.parse_nested_meta(|meta| {
            let name = meta.path.get_ident().map(|i| i.to_string()).unwrap_or_default();
            match name.as_str() {
                "key" => out.key = true,
                "hidden" => out.hidden = true,
                "label" => out.label = Some(meta.value()?.parse::<syn::LitStr>()?.value()),
                "min" => out.min = Some(number(&meta.value()?.parse::<Expr>()?)?),
                "max" => out.max = Some(number(&meta.value()?.parse::<Expr>()?)?),
                "step" => out.step = Some(number(&meta.value()?.parse::<Expr>()?)?),
                _ => return Err(meta.error("unknown #[ui] option (label, min, max, step, key, hidden)")),
            }
            Ok(())
        })?;
    }
    Ok(out)
}

/// `#[serde(rename_all = "...")]` / `#[serde(rename = "...")]` value, if any.
fn serde_str(attrs: &[Attribute], name: &str) -> Option<String> {
    let mut found = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident(name) {
                if let Ok(v) = meta.value().and_then(|v| v.parse::<syn::LitStr>()) {
                    found = Some(v.value());
                }
            } else if meta.input.peek(syn::Token![=]) {
                // Skip the value of an option we don't care about.
                let _ = meta.value().and_then(|v| v.parse::<Expr>());
            }
            Ok(())
        });
    }
    found
}

fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn doc_comment(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("doc")) {
        if let Meta::NameValue(nv) = &attr.meta {
            if let Expr::Lit(ExprLit { lit: Lit::Str(s), .. }) = &nv.value {
                lines.push(s.value().trim().to_string());
            }
        }
    }
    lines.join(" ").trim().to_string()
}

/// The last path segment's name and its first generic argument, if any.
fn type_name(ty: &Type) -> Option<(String, Option<&Type>)> {
    let Type::Path(path) = ty else { return None };
    let segment = path.path.segments.last()?;
    let arg = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args.args.iter().find_map(|a| match a {
            syn::GenericArgument::Type(t) => Some(t),
            _ => None,
        }),
        _ => None,
    };
    Some((segment.ident.to_string(), arg))
}

fn expand(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let name = &input.ident;
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new_spanned(input, "ConfigUi: only structs are supported"));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new_spanned(input, "ConfigUi: only structs with named fields"));
    };
    let container = ui_attrs(&input.attrs)?;
    let rename_all = serde_str(&input.attrs, "rename_all");
    if let Some(rule) = &rename_all {
        if rule != "PascalCase" {
            return Err(syn::Error::new_spanned(name, "ConfigUi: only #[serde(rename_all = \"PascalCase\")] is supported"));
        }
    }

    let mut pushes = Vec::new();
    for field in &fields.named {
        let ident = field.ident.as_ref().unwrap();
        let attrs = ui_attrs(&field.attrs)?;
        if attrs.hidden {
            continue;
        }
        let key = serde_str(&field.attrs, "rename").unwrap_or_else(|| match rename_all {
            Some(_) => pascal_case(&ident.to_string()),
            None => ident.to_string(),
        });
        let label = attrs.label.clone().unwrap_or_else(|| key.clone());
        let description = doc_comment(&field.attrs);
        let min = attrs.min.or(container.min);
        let max = attrs.max.or(container.max);
        let step = attrs.step.or(container.step);

        let Some((type_ident, inner)) = type_name(&field.ty) else {
            return Err(syn::Error::new_spanned(&field.ty, "ConfigUi: unsupported field type"));
        };
        let kind = match type_ident.as_str() {
            "f32" | "f64" => {
                let (min, max, step) = (min.unwrap_or(0.0), max.unwrap_or(10.0), step.unwrap_or(0.01));
                quote! { ::common::menu_schema::UiKind::Float { min: #min, max: #max, step: #step } }
            }
            "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "isize" => {
                let (min, max) = (min.unwrap_or(0.0) as i64, max.unwrap_or(100.0) as i64);
                quote! { ::common::menu_schema::UiKind::Int { min: #min, max: #max } }
            }
            "bool" => quote! { ::common::menu_schema::UiKind::Bool },
            "String" if attrs.key => quote! { ::common::menu_schema::UiKind::Key },
            "String" => quote! { ::common::menu_schema::UiKind::Text },
            "Option" => continue,
            "Vec" => {
                let Some(item) = inner else {
                    return Err(syn::Error::new_spanned(&field.ty, "ConfigUi: Vec without an item type"));
                };
                quote! { ::common::menu_schema::UiKind::List { item: <#item as ::common::menu_schema::ConfigUi>::ui_fields() } }
            }
            _ => {
                let ty = &field.ty;
                quote! { ::common::menu_schema::UiKind::Group { fields: <#ty as ::common::menu_schema::ConfigUi>::ui_fields() } }
            }
        };
        pushes.push(quote! {
            fields.push(::common::menu_schema::UiField {
                key: #key.to_string(),
                label: #label.to_string(),
                description: #description.to_string(),
                kind: #kind,
            });
        });
    }

    Ok(quote! {
        impl ::common::menu_schema::ConfigUi for #name {
            fn ui_fields() -> ::std::vec::Vec<::common::menu_schema::UiField> {
                let mut fields = ::std::vec::Vec::new();
                #(#pushes)*
                fields
            }
        }
    })
}
