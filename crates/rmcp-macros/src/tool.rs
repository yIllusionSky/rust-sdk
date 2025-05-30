use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{ToTokens, quote};
use serde_json::json;
use syn::{
    Expr, FnArg, Ident, ItemFn, ItemImpl, MetaList, PatType, Token, Type, Visibility,Lit,
    parse::{Parse, discouraged::Speculative},
    parse_quote,
    spanned::Spanned,
};

/// Stores tool annotation attributes
#[derive(Default, Clone)]
struct ToolAnnotationAttrs(pub serde_json::Map<String, serde_json::Value>);

impl Parse for ToolAnnotationAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut attrs = serde_json::Map::new();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            let value: Lit = input.parse()?;
            let value = match value {
                Lit::Str(s) => json!(s.value()),
                Lit::Bool(b) => json!(b.value),
                _ => {
                    return Err(syn::Error::new(
                        key.span(),
                        "annotations must be string or boolean literals",
                    ));
                }
            };
            attrs.insert(key.to_string(), value);
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(ToolAnnotationAttrs(attrs))
    }
}

#[derive(Default)]
pub(crate) struct ToolImplItemAttrs {
    tool_box: Option<Option<Ident>>,
    default_build: bool,
    description: Option<Expr>,
}

impl Parse for ToolImplItemAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut tool_box = None;
        let mut default = true;
        let mut description = None;
        while !input.is_empty() {
            let key: Ident = input.parse()?;
            match key.to_string().as_str() {
                "tool_box" => {
                    tool_box = Some(None);
                    if input.lookahead1().peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: Ident = input.parse()?;
                        tool_box = Some(Some(value));
                    }
                }
                "default_build" => {
                    if input.lookahead1().peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: Expr = input.parse()?;
                        match value.to_token_stream().to_string().as_str() {
                            "true" => {
                                default = true;
                            }
                            "false" => {
                                default = false;
                            }
                            _ => {
                                return Err(syn::Error::new(key.span(), "unknown attribute"));
                            }
                        }
                    } else {
                        default = true;
                    }
                }
                "description" => {
                    if input.lookahead1().peek(Token![=]) {
                        input.parse::<Token![=]>()?;
                        let value: Expr = input.parse()?;
                        description = Some(value);
                    }
                }
                _ => {
                    return Err(syn::Error::new(key.span(), "unknown attribute"));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(ToolImplItemAttrs {
            tool_box,
            default_build: default,
            description,
        })
    }
}

#[derive(Default)]
struct ToolFnItemAttrs {
    name: Option<Expr>,
    description: Option<Expr>,
    vis: Option<Visibility>,
    aggr: bool,
    annotations: Option<ToolAnnotationAttrs>,
}

impl Parse for ToolFnItemAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut vis = None;
        let mut aggr = false;
        let mut annotations = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            let key_str = key.to_string();
            if key_str == AGGREGATED_IDENT {
                aggr = true;
                continue;
            }
            input.parse::<Token![=]>()?;
            match key_str.as_str() {
                "name" => {
                    let value: Expr = input.parse()?;
                    name = Some(value);
                }
                "description" => {
                    let value: Expr = input.parse()?;
                    description = Some(value);
                }
                "vis" => {
                    let value: Visibility = input.parse()?;
                    vis = Some(value);
                }
                "annotations" => {
                    // Parse the annotations as a nested structure
                    let content;
                    syn::braced!(content in input);
                    let value = content.parse()?;
                    annotations = Some(value);
                }
                _ => {
                    return Err(syn::Error::new(key.span(), "unknown attribute"));
                }
            }
            if input.is_empty() {
                break;
            }
            input.parse::<Token![,]>()?;
        }

        Ok(ToolFnItemAttrs {
            name,
            description,
            vis,
            aggr,
            annotations,
        })
    }
}

struct ToolFnParamAttrs {
    serde_meta: Vec<MetaList>,
    schemars_meta: Vec<MetaList>,
    ident: Ident,
    rust_type: Box<Type>,
}

impl ToTokens for ToolFnParamAttrs {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = &self.ident;
        let rust_type = &self.rust_type;
        let serde_meta = &self.serde_meta;
        let schemars_meta = &self.schemars_meta;
        tokens.extend(quote! {
            #(#[#serde_meta])*
            #(#[#schemars_meta])*
            pub #ident: #rust_type,
        });
    }
}

#[derive(Default)]

enum ToolParams {
    Aggregated {
        rust_type: PatType,
    },
    Params {
        attrs: Vec<ToolFnParamAttrs>,
    },
    #[default]
    NoParam,
}

#[derive(Default)]
struct ToolAttrs {
    fn_item: ToolFnItemAttrs,
    params: ToolParams,
}
const TOOL_IDENT: &str = "tool";
const SERDE_IDENT: &str = "serde";
const SCHEMARS_IDENT: &str = "schemars";
const PARAM_IDENT: &str = "param";
const AGGREGATED_IDENT: &str = "aggr";
const REQ_IDENT: &str = "req";

pub enum ParamMarker {
    Param,
    Aggregated,
}

impl Parse for ParamMarker {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ident: Ident = input.parse()?;
        match ident.to_string().as_str() {
            PARAM_IDENT => Ok(ParamMarker::Param),
            AGGREGATED_IDENT | REQ_IDENT => Ok(ParamMarker::Aggregated),
            _ => Err(syn::Error::new(ident.span(), "unknown attribute")),
        }
    }
}

pub enum ToolItem {
    Fn(ItemFn),
    Impl(ItemImpl),
}

impl Parse for ToolItem {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let fork = input.fork();
        if let Ok(item) = fork.parse::<ItemImpl>() {
            input.advance_to(&fork);
            return Ok(ToolItem::Impl(item));
        }
        let fork = input.fork();
        if let Ok(item) = fork.parse::<ItemFn>() {
            input.advance_to(&fork);
            return Ok(ToolItem::Fn(item));
        }
        Err(syn::Error::new(
            input.span(),
            "expected function or impl block",
        ))
    }
}

// dispatch impl function item and impl block item
pub(crate) fn tool(attr: TokenStream, input: TokenStream) -> syn::Result<TokenStream> {
    let tool_item = syn::parse2::<ToolItem>(input)?;
    match tool_item {
        ToolItem::Fn(item) => tool_fn_item(attr, item),
        ToolItem::Impl(item) => tool_impl_item(attr, item),
    }
}

pub(crate) fn tool_impl_item(attr: TokenStream, mut input: ItemImpl) -> syn::Result<TokenStream> {
    let tool_impl_attr: ToolImplItemAttrs = syn::parse2(attr)?;
    let tool_box_ident = tool_impl_attr.tool_box;
    let mut extend_quote = None;
    let description = if let Some(expr) = tool_impl_attr.description {
        // Use explicitly provided description if available
        expr
    } else {
        // Try to extract documentation comments
        let doc_content = input
            .attrs
            .iter()
            .filter_map(extract_doc_line)
            .collect::<Vec<_>>()
            .join("\n");
        parse_quote! {
                #doc_content.trim().to_string()
        }
    };
    // get all tool function ident
    let mut tool_fn_idents = Vec::new();
    for item in &input.items {
        if let syn::ImplItem::Fn(method) = item {
            for attr in &method.attrs {
                if attr.path().is_ident(TOOL_IDENT) {
                    tool_fn_idents.push(method.sig.ident.clone());
                }
            }
        }
    }

    // handle different cases
    if input.trait_.is_some() {
        if let Some(ident) = tool_box_ident {
            // check if there are generic parameters
            if !input.generics.params.is_empty() {
                // for trait implementation with generic parameters, directly use the already generated *_inner method

                // generate call_tool method
                input.items.push(parse_quote! {
                    async fn call_tool(
                        &self,
                        request: rmcp::model::CallToolRequestParam,
                        context: rmcp::service::RequestContext<rmcp::RoleServer>,
                    ) -> Result<rmcp::model::CallToolResult, rmcp::Error> {
                        self.call_tool_inner(request, context).await
                    }
                });

                // generate list_tools method
                input.items.push(parse_quote! {
                    async fn list_tools(
                        &self,
                        request: rmcp::model::PaginatedRequestParam,
                        context: rmcp::service::RequestContext<rmcp::RoleServer>,
                    ) -> Result<rmcp::model::ListToolsResult, rmcp::Error> {
                        self.list_tools_inner(request, context).await
                    }
                });
            } else {
                // if there are no generic parameters, add tool box derive
                input.items.push(parse_quote!(
                    rmcp::tool_box!(@derive #ident);
                ));
            }
        } else {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "tool_box attribute is required for trait implementation",
            ));
        }
    } else if let Some(ident) = tool_box_ident {
        // if it is a normal impl block
        if !input.generics.params.is_empty() {
            // if there are generic parameters, not use tool_box! macro, but generate code directly

            // create call code for each tool function
            let match_arms = tool_fn_idents.iter().map(|ident| {
                let attr_fn = Ident::new(&format!("{}_tool_attr", ident), ident.span());
                let call_fn = Ident::new(&format!("{}_tool_call", ident), ident.span());
                quote! {
                    name if name == Self::#attr_fn().name => {
                        Self::#call_fn(tcc).await
                    }
                }
            });

            let tool_attrs = tool_fn_idents.iter().map(|ident| {
                let attr_fn = Ident::new(&format!("{}_tool_attr", ident), ident.span());
                quote! { Self::#attr_fn() }
            });

            // implement call_tool method
            input.items.push(parse_quote! {
                async fn call_tool_inner(
                    &self,
                    request: rmcp::model::CallToolRequestParam,
                    context: rmcp::service::RequestContext<rmcp::RoleServer>,
                ) -> Result<rmcp::model::CallToolResult, rmcp::Error> {
                    let tcc = rmcp::handler::server::tool::ToolCallContext::new(self, request, context);
                    match tcc.name() {
                        #(#match_arms,)*
                        _ => Err(rmcp::Error::invalid_params("tool not found", None)),
                    }
                }
            });

            // implement list_tools method
            input.items.push(parse_quote! {
                async fn list_tools_inner(
                    &self,
                    _: rmcp::model::PaginatedRequestParam,
                    _: rmcp::service::RequestContext<rmcp::RoleServer>,
                ) -> Result<rmcp::model::ListToolsResult, rmcp::Error> {
                    Ok(rmcp::model::ListToolsResult {
                        next_cursor: None,
                        tools: vec![#(#tool_attrs),*],
                    })
                }
            });

            if tool_impl_attr.default_build {
                let struct_name = input.self_ty.clone();
                let generic = &input.generics;
                let extend = quote! {
                    impl #generic rmcp::handler::server::ServerHandler for #struct_name {
                        async fn call_tool(
                            &self,
                            request: rmcp::model::CallToolRequestParam,
                            context: rmcp::service::RequestContext<rmcp::RoleServer>,
                        ) -> Result<rmcp::model::CallToolResult, rmcp::Error> {
                            self.call_tool_inner(request, context).await
                        }
                        async fn list_tools(
                            &self,
                            request: Option<rmcp::model::PaginatedRequestParam>,
                            context: rmcp::service::RequestContext<rmcp::RoleServer>,
                        ) -> Result<rmcp::model::ListToolsResult, rmcp::Error> {
                            self.list_tools_inner(request.unwrap_or_default(), context).await
                        }
                        fn get_info(&self) -> rmcp::model::ServerInfo {
                            rmcp::model::ServerInfo {
                                instructions: Some(#description.into()),
                                capabilities: rmcp::model::ServerCapabilities::builder().enable_tools().build(),
                                ..Default::default()
                            }
                        }
                    }
                };
                extend_quote.replace(extend);
            }
        } else {
            // if there are no generic parameters, use the original tool_box! macro
            let this_type_ident = &input.self_ty;
            input.items.push(parse_quote!(
                rmcp::tool_box!(#this_type_ident {
                    #(#tool_fn_idents),*
                } #ident);
            ));
            if tool_impl_attr.default_build {
                let struct_name = input.self_ty.clone();
                let generic = &input.generics;
                let extend = quote! {
                    impl #generic rmcp::handler::server::ServerHandler for #struct_name {
                        rmcp::tool_box!(@derive #ident);

                        fn get_info(&self) -> rmcp::model::ServerInfo {
                            rmcp::model::ServerInfo {
                                instructions: Some(#description.into()),
                                capabilities: rmcp::model::ServerCapabilities::builder().enable_tools().build(),
                                ..Default::default()
                            }
                        }
                    }
                };
                extend_quote.replace(extend);
            }
        }
    }

    Ok(quote! {
        #input
        #extend_quote
    })
}

// extract doc line from attribute
fn extract_doc_line(attr: &syn::Attribute) -> Option<String> {
    if !attr.path().is_ident("doc") {
        return None;
    }

    let syn::Meta::NameValue(name_value) = &attr.meta else {
        return None;
    };

    let syn::Expr::Lit(expr_lit) = &name_value.value else {
        return None;
    };

    let syn::Lit::Str(lit_str) = &expr_lit.lit else {
        return None;
    };

    let content = lit_str.value().trim().to_string();

    (!content.is_empty()).then_some(content)
}

pub(crate) fn tool_fn_item(attr: TokenStream, mut input_fn: ItemFn) -> syn::Result<TokenStream> {
    let mut tool_macro_attrs = ToolAttrs::default();
    let args: ToolFnItemAttrs = syn::parse2(attr)?;
    tool_macro_attrs.fn_item = args;
    // let mut fommated_fn_args: Punctuated<FnArg, Comma> = Punctuated::new();
    let mut unextractable_args_indexes = HashSet::new();
    for (index, mut fn_arg) in input_fn.sig.inputs.iter_mut().enumerate() {
        enum Caught {
            Param(ToolFnParamAttrs),
            Aggregated(PatType),
        }
        let mut caught = None;
        match &mut fn_arg {
            FnArg::Receiver(_) => {
                continue;
            }
            FnArg::Typed(pat_type) => {
                let mut serde_metas = Vec::new();
                let mut schemars_metas = Vec::new();
                let mut arg_ident = match pat_type.pat.as_ref() {
                    syn::Pat::Ident(pat_ident) => Some(pat_ident.ident.clone()),
                    _ => None,
                };
                let raw_attrs: Vec<_> = pat_type.attrs.drain(..).collect();
                for attr in raw_attrs {
                    match &attr.meta {
                        syn::Meta::List(meta_list) => {
                            if meta_list.path.is_ident(SERDE_IDENT) {
                                serde_metas.push(meta_list.clone());
                            } else if meta_list.path.is_ident(SCHEMARS_IDENT) {
                                schemars_metas.push(meta_list.clone());
                            } else {
                                pat_type.attrs.push(attr);
                            }
                        }
                        _ => {
                            pat_type.attrs.push(attr);
                        }
                    }
                }
                let pat_type = pat_type.clone();
                if tool_macro_attrs.fn_item.aggr {
                    caught.replace(Caught::Aggregated(pat_type.clone()));
                } else {
                    let Some(arg_ident) = arg_ident.take() else {
                        return Err(syn::Error::new(
                            proc_macro2::Span::call_site(),
                            "input param must have an ident as name",
                        ));
                    };
                    caught.replace(Caught::Param(ToolFnParamAttrs {
                        serde_meta: Vec::new(),
                        schemars_meta: Vec::new(),
                        ident: arg_ident,
                        rust_type: pat_type.ty.clone(),
                    }));
                }
                match caught {
                    Some(Caught::Param(mut param)) => {
                        param.serde_meta = serde_metas;
                        param.schemars_meta = schemars_metas;
                        match &mut tool_macro_attrs.params {
                            ToolParams::Params { attrs } => {
                                attrs.push(param);
                            }
                            _ => {
                                tool_macro_attrs.params = ToolParams::Params { attrs: vec![param] };
                            }
                        }
                        unextractable_args_indexes.insert(index);
                    }
                    Some(Caught::Aggregated(rust_type)) => {
                        if let ToolParams::Params { .. } = tool_macro_attrs.params {
                            return Err(syn::Error::new(
                                rust_type.span(),
                                "cannot mix aggregated and individual parameters",
                            ));
                        }
                        tool_macro_attrs.params = ToolParams::Aggregated { rust_type };
                        unextractable_args_indexes.insert(index);
                    }
                    None => {}
                }
            }
        }
    }

    // input_fn.sig.inputs = fommated_fn_args;
    let name = if let Some(expr) = tool_macro_attrs.fn_item.name {
        expr
    } else {
        let fn_name = &input_fn.sig.ident;
        parse_quote! {
            stringify!(#fn_name)
        }
    };
    let tool_attr_fn_ident = Ident::new(
        &format!("{}_tool_attr", input_fn.sig.ident),
        proc_macro2::Span::call_site(),
    );

    // generate get tool attr function
    let tool_attr_fn = {
        let description = if let Some(expr) = tool_macro_attrs.fn_item.description {
            // Use explicitly provided description if available
            expr
        } else {
            // Try to extract documentation comments
            let doc_content = input_fn
                .attrs
                .iter()
                .filter_map(extract_doc_line)
                .collect::<Vec<_>>()
                .join("\n");
            parse_quote! {
                    #doc_content.trim().to_string()
            }
        };
        let schema = match &tool_macro_attrs.params {
            ToolParams::Aggregated { rust_type } => {
                let ty = &rust_type.ty;
                let schema = quote! {
                    rmcp::handler::server::tool::cached_schema_for_type::<#ty>()
                };
                schema
            }
            ToolParams::Params { attrs, .. } => {
                let (param_type, temp_param_type_name) =
                    create_request_type(attrs, input_fn.sig.ident.to_string());
                let schema = quote! {
                    {
                        #param_type
                        rmcp::handler::server::tool::cached_schema_for_type::<#temp_param_type_name>()
                    }
                };
                schema
            }
            ToolParams::NoParam => {
                quote! {
                    rmcp::handler::server::tool::cached_schema_for_type::<rmcp::model::EmptyObject>()
                }
            }
        };
        let input_fn_attrs = &input_fn.attrs;
        let input_fn_vis = &input_fn.vis;

        let annotations_code = if let Some(annotations) = &tool_macro_attrs.fn_item.annotations {
            let annotations =
                serde_json::to_string(&annotations.0).expect("failed to serialize annotations");
            quote! {
                Some(serde_json::from_str::<rmcp::model::ToolAnnotations>(&#annotations).expect("Could not parse tool annotations"))
            }
        } else {
            quote! { None }
        };

        quote! {
            #(#input_fn_attrs)*
            #input_fn_vis fn #tool_attr_fn_ident() -> rmcp::model::Tool {
                rmcp::model::Tool {
                    name: #name.into(),
                    description: Some(#description.into()),
                    input_schema: #schema.into(),
                    annotations: #annotations_code,
                }
            }
        }
    };

    // generate wrapped tool function
    let tool_call_fn = {
        // wrapper function have the same sig:
        // async fn #tool_tool_call(context: rmcp::handler::server::tool::ToolCallContext<'_, Self>)
        //      -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error>
        //
        // and the block part should be like:
        // {
        //      use rmcp::handler::server::tool::*;
        //      let (t0, context) = <T0>::from_tool_call_context_part(context)?;
        //      let (t1, context) = <T1>::from_tool_call_context_part(context)?;
        //      ...
        //      let (tn, context) = <Tn>::from_tool_call_context_part(context)?;
        //      // for params
        //      ... expand helper types here
        //      let (__rmcp_tool_req, context) = rmcp::model::JsonObject::from_tool_call_context_part(context)?;
        //      let __#TOOL_ToolCallParam { param_0, param_1, param_2, .. } = parse_json_object(__rmcp_tool_req)?;
        //      // for aggr
        //      let (Parameters(aggr), context) = <Parameters<AggrType>>::from_tool_call_context_part(context)?;
        //      Self::#tool_ident(to, param_0, t1, param_1, ..., param_2, tn, aggr).await.into_call_tool_result()
        //
        // }
        //
        //
        //

        // for receiver type, name it as __rmcp_tool_receiver
        let is_async = input_fn.sig.asyncness.is_some();
        let receiver_ident = || Ident::new("__rmcp_tool_receiver", proc_macro2::Span::call_site());
        // generate the extraction part for trivial args
        let trivial_args = input_fn
            .sig
            .inputs
            .iter()
            .enumerate()
            .filter_map(|(index, arg)| {
                if unextractable_args_indexes.contains(&index) {
                    None
                } else {
                    // get ident/type pair
                    let line = match arg {
                        FnArg::Typed(pat_type) => {
                            let pat = &pat_type.pat;
                            let ty = &pat_type.ty;
                            quote! {
                                let (#pat, context) = <#ty>::from_tool_call_context_part(context)?;
                            }
                        }
                        FnArg::Receiver(r) => {
                            let ty = r.ty.clone();
                            let pat = receiver_ident();
                            quote! {
                                let  (#pat, context) = <#ty>::from_tool_call_context_part(context)?;
                            }
                        }
                    };
                    Some(line)
                }
            });
        let trivial_arg_extraction_part = quote! {
            #(#trivial_args)*
        };
        let processed_arg_extraction_part = match &mut tool_macro_attrs.params {
            ToolParams::Aggregated { rust_type } => {
                let PatType { pat, ty, .. } = rust_type;
                quote! {
                    let (Parameters(#pat), context) = <Parameters<#ty>>::from_tool_call_context_part(context)?;
                }
            }
            ToolParams::Params { attrs } => {
                let (param_type, temp_param_type_name) =
                    create_request_type(attrs, input_fn.sig.ident.to_string());

                let params_ident = attrs.iter().map(|attr| &attr.ident).collect::<Vec<_>>();
                quote! {
                    #param_type
                    let (__rmcp_tool_req, context) = rmcp::model::JsonObject::from_tool_call_context_part(context)?;
                    let #temp_param_type_name {
                        #(#params_ident,)*
                    } = parse_json_object(__rmcp_tool_req)?;
                }
            }
            ToolParams::NoParam => {
                quote! {}
            }
        };
        // generate the execution part
        // has receiver?
        let params = &input_fn
            .sig
            .inputs
            .iter()
            .map(|fn_arg| match fn_arg {
                FnArg::Receiver(_) => {
                    let pat = receiver_ident();
                    quote! { #pat }
                }
                FnArg::Typed(pat_type) => {
                    let pat = &pat_type.pat.clone();
                    quote! { #pat }
                }
            })
            .collect::<Vec<_>>();
        let raw_fn_ident = &input_fn.sig.ident;
        let call = if is_async {
            quote! {
                Self::#raw_fn_ident(#(#params),*).await.into_call_tool_result()
            }
        } else {
            quote! {
                Self::#raw_fn_ident(#(#params),*).into_call_tool_result()
            }
        };
        // assemble the whole function
        let tool_call_fn_ident = Ident::new(
            &format!("{}_tool_call", input_fn.sig.ident),
            proc_macro2::Span::call_site(),
        );
        let raw_fn_vis = tool_macro_attrs
            .fn_item
            .vis
            .as_ref()
            .unwrap_or(&input_fn.vis);
        let raw_fn_attr = &input_fn
            .attrs
            .iter()
            .filter(|attr| !attr.path().is_ident(TOOL_IDENT))
            .collect::<Vec<_>>();
        quote! {
            #(#raw_fn_attr)*
            #raw_fn_vis async fn #tool_call_fn_ident(context: rmcp::handler::server::tool::ToolCallContext<'_, Self>)
                -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
                use rmcp::handler::server::tool::*;
                #trivial_arg_extraction_part
                #processed_arg_extraction_part
                #call
            }
        }
    };
    Ok(quote! {
        #tool_attr_fn
        #tool_call_fn
        #input_fn
    })
}

fn create_request_type(attrs: &[ToolFnParamAttrs], tool_name: String) -> (TokenStream, Ident) {
    let pascal_case_tool_name = tool_name.to_ascii_uppercase();
    let temp_param_type_name = Ident::new(
        &format!("__{pascal_case_tool_name}ToolCallParam",),
        proc_macro2::Span::call_site(),
    );
    (
        quote! {
            use rmcp::{serde, schemars};
            #[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
            pub struct #temp_param_type_name {
                #(#attrs)*
            }
        },
        temp_param_type_name,
    )
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_tool_sync_macro() -> syn::Result<()> {
        let attr = quote! {
            name = "test_tool",
            description = "test tool",
            vis =
        };
        let input = quote! {
            fn sum(&self, #[tool(aggr)] req: StructRequest) -> Result<CallToolResult, McpError> {
                Ok(CallToolResult::success(vec![Content::text((req.a + req.b).to_string())]))
            }
        };
        let input = tool(attr, input)?;

        println!("input: {:#}", input);
        Ok(())
    }

    #[test]
    fn test_trait_tool_macro() -> syn::Result<()> {
        let attr = quote! {
            tool_box = Calculator
        };
        let input = quote! {
            impl ServerHandler for Calculator {
                #[tool]
                fn get_info(&self) -> ServerInfo {
                    ServerInfo {
                        instructions: Some("A simple calculator".into()),
                        ..Default::default()
                    }
                }
            }
        };
        let input = tool(attr, input)?;

        println!("input: {:#}", input);
        Ok(())
    }
    #[test]
    fn test_doc_comment_description() -> syn::Result<()> {
        let attr = quote! {}; // No explicit description
        let input = quote! {
            /// This is a test description from doc comments
            /// with multiple lines
            fn test_function(&self) -> Result<(), Error> {
                Ok(())
            }
        };
        let result = tool(attr, input)?;

        // The output should contain the description from doc comments
        let result_str = result.to_string();
        println!("result: {:#}", result_str);
        assert!(result_str.contains("This is a test description from doc comments"));
        assert!(result_str.contains("with multiple lines"));

        Ok(())
    }
    #[test]
    fn test_explicit_description_priority() -> syn::Result<()> {
        let attr = quote! {
            description = "Explicit description has priority"
        };
        let input = quote! {
            /// Doc comment description that should be ignored
            fn test_function(&self) -> Result<(), Error> {
                Ok(())
            }
        };
        let result = tool(attr, input)?;

        // The output should contain the explicit description
        let result_str = result.to_string();
        assert!(result_str.contains("Explicit description has priority"));
        Ok(())
    }
}
