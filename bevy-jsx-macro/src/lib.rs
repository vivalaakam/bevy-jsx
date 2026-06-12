//! Procedural macro implementation of `jsx_component!` for `bevy-jsx`.
//!
//! Lives in a separate crate because `proc-macro = true` crates can only
//! export macros. Users should depend on `bevy-jsx`, which re-exports
//! [`jsx_component!`] — never on this crate directly.
//!
//! See the `bevy-jsx` crate documentation for the full syntax reference.

use proc_macro::TokenStream;
use proc_macro2::{Group, Spacing, Span, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Attribute, Expr, Ident, Token, Type, braced, parenthesized};

// ── Parsed input ──────────────────────────────────────────────────────────

struct FieldDef {
    attrs: Vec<Attribute>,
    name: Ident,
    ty: Type,
    default: Option<Expr>,
}

/// How the build closure binds props.
enum PropsBinding {
    /// `|props| body` — the whole props struct under one name.
    Whole(Ident),
    /// `|{a, b, ..rest}| body` — extract some fields, optionally keep the
    /// rest available through `..rest` spreads inside the body.
    Destructured {
        extracted: Vec<Ident>,
        rest: Option<Ident>,
    },
}

struct JsxComponentInput {
    attrs: Vec<Attribute>,
    name: Ident,
    props_name: Ident,
    fields: Vec<FieldDef>,
    binding: PropsBinding,
    body: TokenStream2,
}

impl Parse for JsxComponentInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let attrs = input.call(Attribute::parse_outer)?;
        let name: Ident = input.parse()?;

        let props_paren;
        parenthesized!(props_paren in input);
        let props_name: Ident = props_paren.parse()?;
        if !props_paren.is_empty() {
            return Err(
                props_paren.error("expected a single props struct name, e.g. `Btn(BtnProps)`")
            );
        }

        let fields_brace;
        braced!(fields_brace in input);
        let mut fields = Vec::new();
        while !fields_brace.is_empty() {
            let f_attrs = fields_brace.call(Attribute::parse_outer)?;
            let f_name: Ident = fields_brace.parse()?;
            fields_brace.parse::<Token![:]>()?;
            let ty: Type = fields_brace.parse()?;
            let default = if fields_brace.peek(Token![=]) {
                fields_brace.parse::<Token![=]>()?;
                Some(fields_brace.parse::<Expr>()?)
            } else {
                None
            };
            if !fields_brace.is_empty() {
                fields_brace.parse::<Token![,]>()?;
            }
            fields.push(FieldDef {
                attrs: f_attrs,
                name: f_name,
                ty,
                default,
            });
        }

        input.parse::<Token![|]>()?;
        let binding = if input.peek(syn::token::Brace) {
            let pat;
            braced!(pat in input);
            let mut extracted = Vec::new();
            let mut rest = None;
            while !pat.is_empty() {
                if pat.peek(Token![..]) {
                    pat.parse::<Token![..]>()?;
                    rest = Some(pat.parse::<Ident>()?);
                    if !pat.is_empty() {
                        return Err(pat.error("`..rest` must be the last element of the pattern"));
                    }
                    break;
                }
                extracted.push(pat.parse::<Ident>()?);
                if !pat.is_empty() {
                    pat.parse::<Token![,]>()?;
                }
            }
            PropsBinding::Destructured { extracted, rest }
        } else {
            PropsBinding::Whole(input.parse()?)
        };
        input.parse::<Token![|]>()?;

        let body: TokenStream2 = input.parse()?;
        if body.is_empty() {
            return Err(input.error("expected a build expression after the closure pattern"));
        }

        Ok(JsxComponentInput {
            attrs,
            name,
            props_name,
            fields,
            binding,
            body,
        })
    }
}

// ── Spread rewriting ──────────────────────────────────────────────────────

/// Recursively replaces every `..<rest_name>` token sequence in `body`
/// with `expansion` (a `field: __jsx_props.field,` list).
fn rewrite_spread(body: TokenStream2, rest_name: &Ident, expansion: &TokenStream2) -> TokenStream2 {
    let tokens: Vec<TokenTree> = body.into_iter().collect();
    let mut out = TokenStream2::new();
    let mut i = 0;
    while i < tokens.len() {
        if let TokenTree::Punct(p1) = &tokens[i]
            && p1.as_char() == '.'
            && p1.spacing() == Spacing::Joint
            && let (Some(TokenTree::Punct(p2)), Some(TokenTree::Ident(id))) =
                (tokens.get(i + 1), tokens.get(i + 2))
            && p2.as_char() == '.'
            && p2.spacing() == Spacing::Alone
            && id == rest_name
        {
            out.extend(expansion.clone());
            i += 3;
            if let Some(TokenTree::Punct(p)) = tokens.get(i)
                && p.as_char() == ','
            {
                i += 1;
            }
            continue;
        }
        if let TokenTree::Group(g) = &tokens[i] {
            let inner = rewrite_spread(g.stream(), rest_name, expansion);
            let mut ng = Group::new(g.delimiter(), inner);
            ng.set_span(g.span());
            out.extend([TokenTree::Group(ng)]);
        } else {
            out.extend([tokens[i].clone()]);
        }
        i += 1;
    }
    out
}

// ── Macro entry point ─────────────────────────────────────────────────────

/// Defines a JSX component: a props struct (with per-field defaults) plus a
/// build function returning `impl Spawnable`.
///
/// Fields **without** `= default_expr` are **required** — they must be
/// provided when calling `element!`. Internally they are stored as
/// `Option<T>` in a hidden "partial props" struct; `build()` unwraps them
/// with a descriptive panic if a required prop was omitted.
///
/// Fields **with** `= default_expr` are **optional** — they get the specified
/// default and can be omitted when calling `element!`.
#[proc_macro]
pub fn jsx_component(input: TokenStream) -> TokenStream {
    let JsxComponentInput {
        attrs,
        name,
        props_name,
        fields,
        binding,
        body,
    } = syn::parse_macro_input!(input as JsxComponentInput);

    let partial_name = format_ident!("__JsxPartial{}", props_name);

    let field_decls = fields.iter().map(|f| {
        let FieldDef {
            attrs, name, ty, ..
        } = f;
        quote! { #(#attrs)* pub #name: #ty, }
    });

    // Partial props: what `element!` fills in. Required fields are stored as
    // `Option<T>` so an unset field is representable without panicking in
    // `Default::default()`.
    let partial_field_decls = fields.iter().map(|f| {
        let FieldDef { name, ty, .. } = f;
        match &f.default {
            Some(_) => quote! { pub #name: #ty, },
            None => quote! { pub #name: ::core::option::Option<#ty>, },
        }
    });

    let partial_field_defaults = fields.iter().map(|f| {
        let name = &f.name;
        match &f.default {
            Some(expr) => quote! { #name: #expr, },
            None => quote! { #name: ::core::option::Option::None, },
        }
    });

    // Uniform setters so `element!` does not need to know which fields are
    // required: `_jsx_props.field(value)` works for both kinds.
    let partial_setters = fields.iter().map(|f| {
        let FieldDef { name, ty, .. } = f;
        let body = match &f.default {
            Some(_) => quote! { self.#name = value; },
            None => quote! { self.#name = ::core::option::Option::Some(value); },
        };
        quote! {
            #[doc(hidden)]
            pub fn #name(&mut self, value: #ty) { #body }
        }
    });

    // build() resolves the partial struct into the public props struct,
    // panicking with a clear message if a required prop was omitted.
    let resolve_fields = fields.iter().map(|f| {
        let fname = &f.name;
        match &f.default {
            Some(_) => quote! { #fname: __jsx_partial_props.#fname, },
            None => {
                let msg = format!("missing required prop `{fname}` for `{name}`");
                quote! { #fname: __jsx_partial_props.#fname.expect(#msg), }
            }
        }
    });

    // Resolve the closure binding
    let (arg_ident, prelude, body) = match binding {
        PropsBinding::Whole(ident) => (ident, TokenStream2::new(), body),
        PropsBinding::Destructured { extracted, rest } => {
            for ident in &extracted {
                if !fields.iter().any(|f| &f.name == ident) {
                    return syn::Error::new(
                        ident.span(),
                        format!("`{ident}` is not a declared prop of `{props_name}`"),
                    )
                    .to_compile_error()
                    .into();
                }
            }

            let arg = Ident::new("__jsx_props", Span::call_site());
            let prelude = if extracted.is_empty() {
                TokenStream2::new()
            } else {
                quote! { let #props_name { #(#extracted,)* .. } = #arg; }
            };

            let body = match rest {
                Some(rest_name) => {
                    let rest_fields: Vec<&Ident> = fields
                        .iter()
                        .map(|f| &f.name)
                        .filter(|&name| !extracted.contains(name))
                        .collect();
                    let expansion = quote! { #(#rest_fields: #arg.#rest_fields,)* };
                    rewrite_spread(body, &rest_name, &expansion)
                }
                None => body,
            };
            (arg, prelude, body)
        }
    };

    let expanded = quote! {
        #(#attrs)*
        #[derive(Debug, Clone)]
        pub struct #props_name {
            #(#field_decls)*
        }

        #[doc(hidden)]
        #[derive(Debug, Clone)]
        pub struct #partial_name {
            #(#partial_field_decls)*
        }

        impl ::core::default::Default for #partial_name {
            fn default() -> Self {
                Self { #(#partial_field_defaults)* }
            }
        }

        impl #partial_name {
            #(#partial_setters)*
        }

        #(#attrs)*
        pub struct #name;

        impl ::bevy_jsx::__JsxComponent for #name {
            type Props = #partial_name;

            #[allow(unused_variables)]
            fn build(__jsx_partial_props: Self::Props) -> impl ::bevy_jsx::Spawnable {
                let #arg_ident = #props_name { #(#resolve_fields)* };
                #prelude
                #body
            }
        }
    };

    expanded.into()
}
