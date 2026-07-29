use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::token::Paren;
use syn::{GenericArgument, PathArguments, Token, Type, parse_macro_input};

pub fn declare_worker_rpc(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as RpcDefinition);

    let query_variants = input.items.iter().map(|rpc| {
        let name = &rpc.variant;
        if rpc.args.is_empty() {
            quote! { #name }
        } else {
            let types = rpc.args.iter().map(|arg| match arg {
                Arg::Normal { ty, .. } => ty,
                Arg::Into { ty, .. } => ty,
            });

            quote! {
                #name(#(#types),*)
            }
        }
    });

    let response_variants = input.items.iter().map(|rpc| {
        let doc_hidden = rpc.private.then(|| quote! { #[doc(hidden)] });
        let name = &rpc.variant;
        let ret = &rpc.ret;
        quote! {
            #doc_hidden
            #name(crate::error::GeneralResult<#ret>)
        }
    });

    let funcs = input.items.iter()
        .filter(|rpc| rpc.method != "_virtual")
        .map(|rpc| {
            let func_name = &rpc.method;
            let func_args = rpc.args.iter().map(|arg| {
                let name = arg.get_name();
                let ty = arg.get_ty();
                match arg.is_into() {
                    true => quote! { #name: impl Into<#ty> },
                    false => quote! { #name: #ty }
                }
            });

            let variant_name = &rpc.variant;
            let query_args = rpc.args.iter().map(|arg| {
                let name = arg.get_name();
                if arg.is_into() {
                    quote! { #name.into() }
                } else {
                    quote! { #name }
                }
            });

            let doc_hidden = rpc.private.then(|| quote! { #[doc(hidden)] });
            let fn_visibility = rpc.private
                .then(|| quote! { pub(crate) })
                .unwrap_or_else(|| quote! { pub });

            let query_variant = if !rpc.args.is_empty() {
                quote! {
                    #doc_hidden
                    #variant_name(#(#query_args),*)
                }
            } else {
                quote! {
                    #doc_hidden
                    #variant_name
                }
            };

            let ret_ty = &rpc.ret;

            quote! {
                impl crate::worker::PduAsyncWorker {
                    #doc_hidden
                    #fn_visibility async fn #func_name(&self, #(#func_args),*) -> crate::error::GeneralResult<#ret_ty> {
                        match self.receive_query_response_callback(Query::#query_variant).await? {
                            Response::#variant_name(v) => Ok(v?),
                            _ => unreachable!()
                        }
                    }
                }
            }
        });

    quote! {
        pub enum Query {
            #(#query_variants),*
        }

        pub enum Response {
            #(#response_variants),*
        }

        #(#funcs)*
    }
    .into()
}

struct RpcDefinition {
    pub items: Vec<Rpc>,
}

impl Parse for RpcDefinition {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();

        while !input.is_empty() {
            let variant: Ident = input.parse().map_err(|_| {
                syn::Error::new(
                    input.span(),
                    "expected RPC variant name, e.g. PduGetEventItem",
                )
            })?;

            input.parse::<Token![=>]>().map_err(|_| {
                syn::Error::new(input.span(), "expected RPC variant name delimiter")
            })?;

            let private = if input.peek(Token![!]) {
                // private query/function
                input.parse::<Token![!]>()?;
                true
            } else {
                false
            };

            let method: Ident = input.parse().map_err(|_| {
                syn::Error::new(
                    input.span(),
                    "expected RPC function name, e.g. pdu_get_event_item",
                )
            })?;

            let args_content; // args

            if !input.peek(Paren) {
                return Err(syn::Error::new(
                    input.span(),
                    "expected `(...)` after RPC function name",
                ));
            }

            syn::parenthesized!(args_content in input); // (...)

            let mut args = Vec::new();

            while !args_content.is_empty() {
                let name: Ident = args_content.parse().map_err(|_| {
                    syn::Error::new(
                        args_content.span(),
                        "expected a name of RPC function argument",
                    )
                })?;

                args_content.parse::<Token![:]>().map_err(|_| {
                    syn::Error::new(
                        args_content.span(),
                        "expected semicolon after RPC function name",
                    )
                })?;

                let ty: Type = args_content.parse().map_err(|_| {
                    syn::Error::new(
                        args_content.span(),
                        "expected a type of RPC function argument",
                    )
                })?;

                args.push(match parse_into_type(&ty) {
                    Some(ty) => Arg::Into { name, ty },
                    None => Arg::Normal { name, ty },
                });

                if args_content.peek(Token![,]) {
                    args_content.parse::<Token![,]>()?;
                }
            }

            input.parse::<Token![->]>().map_err(|_| {
                syn::Error::new(input.span(), "expected `->` followed by RPC return type")
            })?;

            let ret: Type = input.parse().map_err(|_| {
                syn::Error::new(input.span(), "expected a type of RPC function return")
            })?;

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }

            items.push(Rpc {
                private,
                variant,
                method,
                args,
                ret,
            });
        }

        Ok(Self { items })
    }
}

struct Rpc {
    private: bool,
    variant: Ident,
    method: Ident,
    args: Vec<Arg>,
    ret: Type,
}

enum Arg {
    Normal { name: Ident, ty: Type },

    Into { name: Ident, ty: Type },
}

impl Arg {
    fn get_name(&self) -> &Ident {
        match self {
            Arg::Normal { name, .. } => name,
            Arg::Into { name, .. } => name,
        }
    }

    fn is_into(&self) -> bool {
        matches!(self, Arg::Into { .. })
    }

    fn get_ty(&self) -> &Type {
        match self {
            Arg::Normal { ty, .. } => ty,
            Arg::Into { ty, .. } => ty,
        }
    }
}

fn parse_into_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let seg = type_path.path.segments.last()?;
    if seg.ident != "Into" {
        return None;
    }

    let PathArguments::AngleBracketed(type_generic) = &seg.arguments else {
        return None;
    };

    match type_generic.args.first()? {
        GenericArgument::Type(inner) => Some(inner.clone()),
        _ => None,
    }
}
