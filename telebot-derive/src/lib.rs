#![feature(proc_macro)]
#![recursion_limit="150"]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use std::collections::BTreeMap;

#[proc_macro_derive(setter)]
pub fn derive_setter(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input(&input.to_string()).unwrap();
    let expanded = expand_setter(ast);
    expanded.to_string().parse().unwrap()
}

fn expand_setter(ast: syn::MacroInput) -> quote::Tokens {
    let config = config_from(&ast.attrs);

    let query_kind = config.get("query").map(|tmp| syn::Lit::from(tmp.as_str()));

    let fields: Vec<_> = match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(ref fields)) => {
            fields
                .iter()
                .map(|f| (f.ident.as_ref().unwrap(), &f.ty))
                .collect()
        }
        syn::Body::Struct(syn::VariantData::Unit) => vec![],
        _ => panic!("#[derive(getters)] can only be used with braced structs"),
    };

    let name = &ast.ident;
    let is_option_ident = |ref f: &(&syn::Ident, &syn::Ty)| -> bool {
        match *f.1 {
            syn::Ty::Path(_, ref path) => {
                match path.segments.first().unwrap().ident.as_ref() {
                    "Option" => true,
                    _ => false,
                }
            }
            _ => false,
        }
    };

    let field_compulsory: Vec<_> = fields
        .iter()
        .filter(|f| !is_option_ident(&f))
        .filter(|f| f.0.as_ref() != "kind" && f.0.as_ref() != "id")
        .map(|f| syn::Ident::from(format!("_{}", f.0.as_ref())))
        .collect();

    let field_optional: Vec<_> = fields
        .iter()
        .filter(|f| is_option_ident(&f))
        .map(|f| f.0)
        .collect();
    let field_optional2 = field_optional.clone();

    let field_compulsory2: Vec<_> = fields
        .iter()
        .map(|f| f.0)
        .filter(|f| f.as_ref() != "kind" && f.as_ref() != "id")
        .collect();


    let field_compulsory3 = field_compulsory.clone();
    let values: Vec<_> = fields
        .iter()
        .filter(|f| f.0.as_ref() != "kind" && f.0.as_ref() != "id")
        .map(|f| match *f.1 {
            syn::Ty::Path(_, ref path) => {
                match path.segments.first().unwrap().ident.as_ref() {
                    "Option" => return syn::Ident::from("None"),
                    _ => return syn::Ident::from(format!("_{}", f.0.as_ref())),
                }
            }
            _ => return syn::Ident::from("None"),
        })
        .collect();

    let ty_compulsory: Vec<_> = fields
        .iter()
        .filter(|f| f.0.as_ref() != "kind" && f.0.as_ref() != "id")
        .map(|f| f.1)
        .collect();
    let ty_optional: Vec<_> = fields
        .iter()
        .filter(|f| is_option_ident(&f))
        .map(|f| {
            if let syn::Ty::Path(_, ref path) = *f.1 {
                if let syn::PathParameters::AngleBracketed(ref param) =
                    path.segments.first().unwrap().parameters
                {
                    if let &syn::Ty::Path(_, ref path) = param.types.first().unwrap() {
                        return (*path).clone();
                    }
                }
            }

            panic!("no sane type!");
        })
        .collect();

    //println!("{:?}", ty_optional.first());

    if let Some(query_name) = query_kind {
        quote! {
            impl #name {
                #[allow(dead_code)]
                pub fn new(#( #field_compulsory3: #ty_compulsory, )*) -> #name {
                    let id = Uuid::new_v4();

                    #name {
                        kind: #query_name.into(),
                        id: id.hyphenated().to_string(),
                        #( #field_compulsory2: #values, )*
                    }
                }
                #(
                    pub fn #field_optional<S>(mut self, val: S)
                                              -> Self where S: Into<#ty_optional> {
                        self.#field_optional2 = Some(val.into());

                        self
                    }
                )*
            }

        }
    } else {
        quote! {
            impl #name {
                #[allow(dead_code)]
                pub fn new(#( #field_compulsory3: #ty_compulsory, )*) -> #name {
                    #name { #( #field_compulsory2: #values, )* }
                }
                #(
                    pub fn #field_optional<S>(mut self, val: S)
                                              -> Self where S: Into<#ty_optional> {
                        self.#field_optional2 = Some(val.into());

                        self
                    }
                )*
            }
        }
    }
}

#[proc_macro_derive(TelegramFunction)]
pub fn derive_telegram_sendable(input: TokenStream) -> TokenStream {
    let ast = syn::parse_macro_input(&input.to_string()).unwrap();
    let expanded = expand_function(ast);
    expanded.to_string().parse().unwrap()
}

fn expand_function(ast: syn::MacroInput) -> quote::Tokens {
    let config = config_from(&ast.attrs);

    let function = config.get("call").unwrap();
    let function = syn::Lit::Str((*function).clone(), syn::StrStyle::Cooked);
    let bot_function = syn::Ident::from(config.get("function").unwrap().as_str());
    let answer = syn::Ident::from(config.get("answer").unwrap().as_str());

    let fields: Vec<_> = match ast.body {
        syn::Body::Struct(syn::VariantData::Struct(ref fields)) => {
            fields
                .iter()
                .map(|f| (f.ident.as_ref().unwrap(), &f.ty))
                .collect()
        }
        syn::Body::Struct(syn::VariantData::Unit) => vec![],
        _ => panic!("#[derive(getters)] can only be used with braced structs"),
    };


    /*for field in &fields {
        println!("{:?}", field.1);
    }*/

    let name = &ast.ident;
    let is_option_ident = |ref f: &(&syn::Ident, &syn::Ty)| -> bool {
        match *f.1 {
            syn::Ty::Path(_, ref path) => {
                match path.segments.first().unwrap().ident.as_ref() {
                    "Option" => true,
                    _ => false,
                }
            }
            _ => false,
        }
    };

    let field_compulsory: Vec<_> = fields
        .iter()
        .filter(|f| !is_option_ident(&f))
        .map(|f| syn::Ident::from(format!("_{}", f.0.as_ref())))
        .collect();

    let field_optional: Vec<_> = fields
        .iter()
        .filter(|f| is_option_ident(&f))
        .map(|f| f.0)
        .collect();
    let field_optional2 = field_optional.clone();

    let field_compulsory3: Vec<_> = fields.iter().map(|f| f.0).collect();
    let field_compulsory2 = field_compulsory.clone();
    let values: Vec<_> = fields
        .iter()
        .map(|f| match *f.1 {
            syn::Ty::Path(_, ref path) => {
                match path.segments.first().unwrap().ident.as_ref() {
                    "Option" => return syn::Ident::from("None"),
                    _ => return syn::Ident::from(format!("_{}", f.0.as_ref())),
                }
            }
            _ => return syn::Ident::from("None"),
        })
        .collect();

    let ty_compulsory: Vec<_> = fields
        .iter()
        .filter(|f| !is_option_ident(&f))
        .map(|f| f.1)
        .collect();
    let ty_compulsory2 = ty_compulsory.clone();
    let ty_compulsory_generic: Vec<_> = (0..ty_compulsory.len())
        .map(|t| syn::Ident::from(format!("T{}", t)))
        .collect();
    let ty_compulsory_generic2 = ty_compulsory_generic.clone();
    let ty_compulsory_generic3 = ty_compulsory_generic.clone();
    let ty_compulsory_generic4 = ty_compulsory_generic.clone();
    let ty_compulsory_generic5 = ty_compulsory_generic.clone();
    let ty_compulsory_generic6 = ty_compulsory_generic.clone();
    let ty_optional: Vec<_> = fields
        .iter()
        .filter(|f| is_option_ident(&f))
        .map(|f| {
            if let syn::Ty::Path(_, ref path) = *f.1 {
                if let syn::PathParameters::AngleBracketed(ref param) =
                    path.segments.first().unwrap().parameters
                {
                    if let &syn::Ty::Path(_, ref path) = param.types.first().unwrap() {
                        return (*path).clone();
                    }
                }
            }

            panic!("no sane type!");
        })
        .collect();

    //println!("{:?}", ty_optional.first());

    let trait_name = syn::Ident::from(format!("Function{}", name.as_ref()));
    let wrapper_name = syn::Ident::from(format!("Wrapper{}", name.as_ref()));

    quote! {
        #[allow(dead_code)]
        pub struct #wrapper_name {
            bot: Rc<Bot>,
            inner: #name,
        }

        pub trait #trait_name {
            fn #bot_function<#( #ty_compulsory_generic, )*>
                (&self, #( #field_compulsory: #ty_compulsory_generic2, )*)
                 -> #wrapper_name
                where #( #ty_compulsory_generic3: Into<#ty_compulsory>, )*;
        }

        impl #trait_name for RcBot {
            fn #bot_function<#( #ty_compulsory_generic4, )*>
                (&self, #( #field_compulsory2: #ty_compulsory_generic5, )*)
                 -> #wrapper_name
                where #( #ty_compulsory_generic6: Into<#ty_compulsory2>, )*
            {
                #wrapper_name {
                    inner: #name {
                        #( #field_compulsory3: #values.into(), )*
                    },
                    bot: self.inner.clone(),
                }
            }
        }

        impl #wrapper_name {
            pub fn send<'a>(self)
                            -> impl Future<Item=(RcBot, objects::#answer), Error=Error> + 'a {
                let msg = serde_json::to_string(&self.inner).unwrap();
                self.bot.fetch_json(#function, &msg)
                    .map(move |x| (RcBot { inner: self.bot.clone() }, x))
            }

            #(
                pub fn #field_optional<S>(mut self, val: S)
                                          -> Self where S: Into<#ty_optional> {
                    self.inner.#field_optional2 = Some(val.into());

                    self
                }
            )*
        }
    }
}

fn config_from(attrs: &[syn::Attribute]) -> BTreeMap<String, String> {
    let mut result = BTreeMap::new();
    for attr in attrs {
        if let syn::MetaItem::NameValue(ref name, ref value) = attr.value {
            let name = format!("{}", name);
            let value = match value.clone() {
                syn::Lit::Str(value, _) => value,
                _ => panic!("bla"),
            };
            result.insert(name, value);
        }
    }
    result
}
