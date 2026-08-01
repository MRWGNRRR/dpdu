use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{format_ident, quote, ToTokens};
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
            let func_name_dedicated = format_ident!("{func_name}_dedicated");
            let func_name_dedicated_str = func_name_dedicated.to_string();

            let func_args = rpc.args.iter().map(|arg| {
                let name = arg.get_name();
                let ty = arg.get_ty();
                match arg.is_into() {
                    true => quote! { #name: impl Into<#ty> },
                    false => quote! { #name: #ty }
                }
            });
            let func_args_dedicated = func_args.clone();

            let variant_name = &rpc.variant;
            let query_args = rpc.args.iter().map(|arg| {
                let name = arg.get_name();
                if arg.is_into() {
                    quote! { #name.into() }
                } else {
                    quote! { #name }
                }
            });

            let make_prepared_dedicated_arg = |ident: &Ident| format_ident!("prep_{ident}");
            let mut prepared_dedicated_args = Vec::<proc_macro2::TokenStream>::new();

            for arg in rpc.args.iter() {
                if arg.is_into() {
                    let arg_name = arg.get_name();
                    let var_name = make_prepared_dedicated_arg(arg.get_name());

                    prepared_dedicated_args.push(quote! {
                        let #var_name = #arg_name.into();
                    });
                }
            }

            let dedicated_args = rpc.args.iter().map(|arg| {
                let name = arg.get_name();
                let dedicated_ref = if arg.is_dedicated_ref() {
                    quote!{ & }
                } else {
                    quote! {}
                };
                if arg.is_into() {
                    let var_name = make_prepared_dedicated_arg(name);
                    quote! { #dedicated_ref #var_name }
                } else {
                    if is_naive_ptr_type(arg.get_ty()) {
                        quote! { #name.as_mut_ptr() }
                    } else {
                        if arg.is_dedicated_opt_ref() {
                            quote! { #name.as_ref() }
                        } else {
                            quote! { #dedicated_ref #name }
                        }
                    }
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

                    #doc_hidden
                    #fn_visibility async fn #func_name_dedicated(&self, #(#func_args_dedicated),*) -> crate::error::GeneralResult<#ret_ty> {
                        #(#prepared_dedicated_args)*
                        let api = self.api.clone();
                        let task = move || api.#func_name(#(#dedicated_args),*);
                        let result = ::tokio::task::spawn_blocking(task)
                            .await
                            .expect(&format!(
                                "internal error: PduAsyncWorker::{}() task panicked",
                                #func_name_dedicated_str
                            ))?;
                        Ok(result)
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

                let mut dedicated_ref = false;
                let mut dedicated_opt_ref = false;

                while args_content.peek(Token![@]) {
                    args_content.parse::<Token![@]>()?; // infallible
                    let ident: Ident = args_content.parse().map_err(|_| {
                        syn::Error::new(
                            args_content.span(),
                            "expected an ident after the @ symbol"
                        )
                    })?;

                    match ident.to_string().as_str() {
                        "dedicated_ref" => { dedicated_ref = true },
                        "dedicated_opt_ref" => { dedicated_opt_ref = true },
                        v => {
                            return Err(syn::Error::new(
                                args_content.span(),
                                format!("unsupported ident after the @ symbol: {v}")
                            ));
                        }
                    }
                }

                let ty: Type = args_content.parse().map_err(|_| {
                    syn::Error::new(
                        args_content.span(),
                        "expected a type of RPC function argument",
                    )
                })?;

                args.push(match parse_into_type(&ty) {
                    Some(ty) => Arg::Into { name, ty, dedicated_ref },
                    None => Arg::Normal { name, ty, dedicated_ref, dedicated_opt_ref },
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
    Normal { name: Ident, ty: Type, dedicated_ref: bool, dedicated_opt_ref: bool },

    Into { name: Ident, ty: Type, dedicated_ref: bool },
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

    fn is_dedicated_ref(&self) -> bool {
        match self {
            Arg::Normal { dedicated_ref, .. } => dedicated_ref,
            Arg::Into { dedicated_ref, .. } => dedicated_ref
        }.to_owned()
    }

    fn is_dedicated_opt_ref(&self) -> bool {
        match self {
            Arg::Normal { dedicated_opt_ref, .. } => dedicated_opt_ref.to_owned(),
            _ => false
        }.to_owned()
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

fn is_vec_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            let path = &path.path;
            path.segments.last()
                .map(|s| s.ident == "Vec")
                .unwrap_or(false)
        },
        _ => false
    }
}

fn is_naive_ptr_type(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => {
            let path = path.path.segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");

            path == "NaivePtr"
        },
        _ => false
    }
}