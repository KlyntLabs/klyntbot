use proc_macro::TokenStream;
use quote::quote;
use syn::parse::Parser;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Lit, Meta};

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let variants = match &input.data {
        Data::Enum(data) => &data.variants,
        _ => panic!("DomainEnum can only be derived for enums"),
    };

    let mut as_str_arms = Vec::new();
    let mut from_str_arms = Vec::new();

    for variant in variants {
        assert!(
            matches!(variant.fields, Fields::Unit),
            "DomainEnum variants must be unit variants"
        );

        let variant_ident = &variant.ident;

        // Check for #[canonical("custom")] override
        let canonical_lower = get_canonical_override(variant).unwrap_or_else(|| {
            // PascalCase → snake_case conversion
            pascal_to_snake(&variant.ident.to_string())
        });

        // as_str arm
        as_str_arms.push(quote! {
            #name::#variant_ident => #canonical_lower,
        });

        // from_str: canonical name
        from_str_arms.push(quote! {
            #canonical_lower => ::core::option::Option::Some(#name::#variant_ident),
        });

        // from_str: aliases
        for attr in &variant.attrs {
            if attr.path().is_ident("aliases") {
                if let Meta::List(list) = &attr.meta {
                    let tokens = list.tokens.clone();
                    let parser =
                        syn::punctuated::Punctuated::<Lit, syn::Token![,]>::parse_terminated;
                    let aliases = parser
                        .parse2(tokens)
                        .expect("Expected string literals in #[aliases(...)]");
                    for lit in aliases {
                        if let Lit::Str(s) = lit {
                            let alias = s.value().to_lowercase();
                            from_str_arms.push(quote! {
                                #alias => ::core::option::Option::Some(#name::#variant_ident),
                            });
                        }
                    }
                }
            }
        }
    }

    let expanded = quote! {
        impl #name {
            pub fn as_str(&self) -> &'static str {
                match self {
                    #(#as_str_arms)*
                }
            }

            pub fn from_str_loose(s: &str) -> ::core::option::Option<Self> {
                match s.to_lowercase().as_str() {
                    #(#from_str_arms)*
                    _ => ::core::option::Option::None,
                }
            }
        }

        impl ::core::fmt::Display for #name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl ::core::str::FromStr for #name {
            type Err = ::std::string::String;

            fn from_str(s: &str) -> ::core::result::Result<Self, Self::Err> {
                Self::from_str_loose(s)
                    .ok_or_else(|| format!("unknown {}: {}", stringify!(#name), s))
            }
        }
    };

    TokenStream::from(expanded)
}

/// Convert PascalCase to snake_case.
fn pascal_to_snake(s: &str) -> String {
    let mut result = String::new();
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.push(c.to_ascii_lowercase());
    }
    result
}

/// Check for `#[canonical("custom")]` attribute on a variant.
fn get_canonical_override(variant: &syn::Variant) -> Option<String> {
    for attr in &variant.attrs {
        if attr.path().is_ident("canonical") {
            if let Meta::List(list) = &attr.meta {
                let tokens = list.tokens.clone();
                if let Ok(Lit::Str(s)) = syn::parse2::<Lit>(tokens) {
                    return Some(s.value());
                }
            }
        }
    }
    None
}
