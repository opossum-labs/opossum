use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, Expr, Lit, Meta, parse_macro_input, punctuated::Punctuated, token::Comma};

#[allow(clippy::too_many_lines)]
#[allow(clippy::cognitive_complexity)]
pub fn impl_derive_validate_numeric(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = input.ident;
    let generics = &input.generics;

    let mut rule_fn = None;
    let mut msg = None;
    let mut target = quote! { Target::Both };
    let mut mode = "all".to_string();
    let mut call_on_self = false;

    for attr in input.attrs.iter().filter(|a| a.path().is_ident("rule")) {
        let metas: Punctuated<Meta, Comma> = attr
            .parse_args_with(Punctuated::parse_terminated)
            .expect("failed to parse #[rule(...)]");

        let mut iter = metas.into_iter();

        // first element: function name
        if let Some(meta) = iter.next() {
            if let Meta::Path(path) = meta {
                rule_fn = path.get_ident().cloned();
            } else if let Meta::NameValue(nv) = meta
                && let Expr::Lit(expr_lit) = &nv.value
                && let Lit::Str(lit_str) = &expr_lit.lit
            {
                rule_fn = Some(syn::Ident::new(&lit_str.value(), lit_str.span()));
            }
        }

        for meta in iter {
            if let Meta::NameValue(nv) = meta {
                if nv.path.is_ident("message")
                    && let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    msg = Some(lit_str.value());
                }

                if nv.path.is_ident("target")
                    && let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    let t = lit_str.value().to_lowercase();
                    target = match t.as_str() {
                        "x" => quote! { Target::X },
                        "y" => quote! { Target::Y },
                        _ => quote! { Target::Both },
                    };
                }

                if nv.path.is_ident("mode")
                    && let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    mode = lit_str.value().to_lowercase();
                }

                if nv.path.is_ident("on")
                    && let Expr::Lit(expr_lit) = &nv.value
                    && let Lit::Str(lit_str) = &expr_lit.lit
                {
                    call_on_self = lit_str.value().to_lowercase() == "self";
                }
            }
        }
    }

    let rule_fn = rule_fn.expect("expected #[rule(method, ...)]");
    let msg_str = msg.unwrap_or_else(|| "Validation failed".to_string());
    let mode_all = mode == "all";
    let is_generic = !generics.params.is_empty();

    // generate two branches depending on call_on_self
    let expanded = if is_generic {
        if call_on_self {
            quote! {
                impl<T: NumLike> Validate<T> for #name<T> {
                    fn validate(&self, val: &T) -> OpmResult<()> {
                        let ok = self.#rule_fn(val);
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                // only for tupels that stor exactly the same type
                impl<T: NumLike> Validate<(T,T)> for #name<T> {
                    fn validate(&self, val: &(T,T)) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => self.#rule_fn(&val.0),
                            Target::Y => self.#rule_fn(&val.1),
                            Target::Both => if #mode_all { self.#rule_fn(&val.0) && self.#rule_fn(&val.1) } else { self.#rule_fn(&val.0) || self.#rule_fn(&val.1) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> Validate<Point2<T>> for #name<T> {
                    fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => self.#rule_fn(&val.x),
                            Target::Y => self.#rule_fn(&val.y),
                            Target::Both => if #mode_all { self.#rule_fn(&val.x) && self.#rule_fn(&val.y) } else { self.#rule_fn(&val.x) || self.#rule_fn(&val.y) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> Validate<Range<T>> for #name<T> {
                    fn validate(&self, val: &Range<T>) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => self.#rule_fn(&val.start),
                            Target::Y => self.#rule_fn(&val.end),
                            Target::Both => if #mode_all { self.#rule_fn(&val.start) && self.#rule_fn(&val.end) } else { self.#rule_fn(&val.start) || self.#rule_fn(&val.end) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike> ValidateVec<T> for #name<T> {
                    fn validate_vec(&self, val: &[T]) -> OpmResult<()> {
                        let ok = if #mode_all { val.iter().all(|v| self.#rule_fn(v)) } else { val.iter().any(|v| self.#rule_fn(v)) };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike> ValidateVec<(T,T)> for #name<T> {
                    fn validate_vec(&self, val: &[(T,T)]) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.0)) } else { val.iter().any(|v| self.#rule_fn(&v.0)) },
                            Target::Y => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.1)) } else { val.iter().any(|v| self.#rule_fn(&v.1)) },
                            Target::Both => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.0) && self.#rule_fn(&v.1)) } else { val.iter().any(|v| self.#rule_fn(&v.0) || self.#rule_fn(&v.1)) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> ValidateVec<Point2<T>> for #name<T> {
                    fn validate_vec(&self, val: &[Point2<T>]) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.x)) } else { val.iter().any(|v| self.#rule_fn(&v.x)) },
                            Target::Y => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.y)) } else { val.iter().any(|v| self.#rule_fn(&v.y)) },
                            Target::Both => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.x) && self.#rule_fn(&v.y)) } else { val.iter().any(|v| self.#rule_fn(&v.x) || self.#rule_fn(&v.y)) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }
            }
        } else {
            // call_on_self = false: use NumLike trait
            quote! {
                impl<T: NumLike> Validate<T> for #name<T> {
                    fn validate(&self, val: &T) -> OpmResult<()> {
                        let ok = NumLike::#rule_fn(val);
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike> Validate<(T,T)> for #name<T> {
                    fn validate(&self, val: &(T,T)) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => NumLike::#rule_fn(&val.0),
                            Target::Y => NumLike::#rule_fn(&val.1),
                            Target::Both => if #mode_all { NumLike::#rule_fn(&val.0) && NumLike::#rule_fn(&val.1) } else { NumLike::#rule_fn(&val.0) || NumLike::#rule_fn(&val.1) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> Validate<Point2<T>> for #name<T> {
                    fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => NumLike::#rule_fn(&val.x),
                            Target::Y => NumLike::#rule_fn(&val.y),
                            Target::Both => if #mode_all { NumLike::#rule_fn(&val.x) && NumLike::#rule_fn(&val.y) } else { NumLike::#rule_fn(&val.x) || NumLike::#rule_fn(&val.y) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> Validate<Range<T>> for #name<T> {
                    fn validate(&self, val: &Range<T>) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => NumLike::#rule_fn(&val.start),
                            Target::Y => NumLike::#rule_fn(&val.end),
                            Target::Both => if #mode_all { NumLike::#rule_fn(&val.start) && NumLike::#rule_fn(&val.end) } else { NumLike::#rule_fn(&val.start) || NumLike::#rule_fn(&val.end) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike> ValidateVec<T> for #name<T> {
                    fn validate_vec(&self, val: &[T]) -> OpmResult<()> {
                        let ok = if #mode_all { val.iter().all(|v| NumLike::#rule_fn(v)) } else { val.iter().any(|v| NumLike::#rule_fn(v)) };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike> ValidateVec<(T,T)> for #name<T> {
                    fn validate_vec(&self, val: &[(T,T)]) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.0)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.0)) },
                            Target::Y => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.1)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.1)) },
                            Target::Both => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.0) && NumLike::#rule_fn(&v.1)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.0) || NumLike::#rule_fn(&v.1)) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }

                impl<T: NumLike + 'static> ValidateVec<Point2<T>> for #name<T> {
                    fn validate_vec(&self, val: &[Point2<T>]) -> OpmResult<()> {
                        let ok = match #target {
                            Target::X => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.x)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.x)) },
                            Target::Y => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.y)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.y)) },
                            Target::Both => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.x) && NumLike::#rule_fn(&v.y)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.x) || NumLike::#rule_fn(&v.y)) },
                        };
                        if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                    }
                }
            }
        }
    } else if call_on_self {
        quote! {
            impl<T: NumLike> Validate<T> for #name {
                fn validate(&self, val: &T) -> OpmResult<()> {
                    let ok = self.#rule_fn(val);
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike, V: NumLike> Validate<(T,V)> for #name {
                fn validate(&self, val: &(T,V)) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => self.#rule_fn(&val.0),
                        Target::Y => self.#rule_fn(&val.1),
                        Target::Both => if #mode_all { self.#rule_fn(&val.0) && self.#rule_fn(&val.1) } else { self.#rule_fn(&val.0) || self.#rule_fn(&val.1) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> Validate<Point2<T>> for #name {
                fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => self.#rule_fn(&val.x),
                        Target::Y => self.#rule_fn(&val.y),
                        Target::Both => if #mode_all { self.#rule_fn(&val.x) && self.#rule_fn(&val.y) } else { self.#rule_fn(&val.x) || self.#rule_fn(&val.y) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> Validate<Range<T>> for #name {
                fn validate(&self, val: &Range<T>) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => self.#rule_fn(&val.start),
                        Target::Y => self.#rule_fn(&val.end),
                        Target::Both => if #mode_all { self.#rule_fn(&val.start) && self.#rule_fn(&val.end) } else { self.#rule_fn(&val.start) || self.#rule_fn(&val.end) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike> ValidateVec<T> for #name {
                fn validate_vec(&self, val: &[T]) -> OpmResult<()> {
                    let ok = if #mode_all { val.iter().all(|v| self.#rule_fn(v)) } else { val.iter().any(|v| self.#rule_fn(v)) };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike, V: NumLike> ValidateVec<(T,V)> for #name {
                fn validate_vec(&self, val: &[(T,V)]) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.0)) } else { val.iter().any(|v| self.#rule_fn(&v.0)) },
                        Target::Y => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.1)) } else { val.iter().any(|v| self.#rule_fn(&v.1)) },
                        Target::Both => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.0) && self.#rule_fn(&v.1)) } else { val.iter().any(|v| self.#rule_fn(&v.0) || self.#rule_fn(&v.1)) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> ValidateVec<Point2<T>> for #name {
                fn validate_vec(&self, val: &[Point2<T>]) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.x)) } else { val.iter().any(|v| self.#rule_fn(&v.x)) },
                        Target::Y => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.y)) } else { val.iter().any(|v| self.#rule_fn(&v.y)) },
                        Target::Both => if #mode_all { val.iter().all(|v| self.#rule_fn(&v.x) && self.#rule_fn(&v.y)) } else { val.iter().any(|v| self.#rule_fn(&v.x) || self.#rule_fn(&v.y)) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }
        }
    } else {
        // call_on_self = false: use NumLike trait
        quote! {
            impl<T: NumLike> Validate<T> for #name {
                fn validate(&self, val: &T) -> OpmResult<()> {
                    let ok = NumLike::#rule_fn(val);
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike, V: NumLike> Validate<(T,V)> for #name {
                fn validate(&self, val: &(T,V)) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => NumLike::#rule_fn(&val.0),
                        Target::Y => NumLike::#rule_fn(&val.1),
                        Target::Both => if #mode_all { NumLike::#rule_fn(&val.0) && NumLike::#rule_fn(&val.1) } else { NumLike::#rule_fn(&val.0) || NumLike::#rule_fn(&val.1) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> Validate<Point2<T>> for #name {
                fn validate(&self, val: &Point2<T>) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => NumLike::#rule_fn(&val.x),
                        Target::Y => NumLike::#rule_fn(&val.y),
                        Target::Both => if #mode_all { NumLike::#rule_fn(&val.x) && NumLike::#rule_fn(&val.y) } else { NumLike::#rule_fn(&val.x) || NumLike::#rule_fn(&val.y) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> Validate<Range<T>> for #name {
                fn validate(&self, val: &Range<T>) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => NumLike::#rule_fn(&val.start),
                        Target::Y => NumLike::#rule_fn(&val.end),
                        Target::Both => if #mode_all { NumLike::#rule_fn(&val.start) && NumLike::#rule_fn(&val.end) } else { NumLike::#rule_fn(&val.start) || NumLike::#rule_fn(&val.end) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike> ValidateVec<T> for #name {
                fn validate_vec(&self, val: &[T]) -> OpmResult<()> {
                    let ok = if #mode_all { val.iter().all(|v| NumLike::#rule_fn(v)) } else { val.iter().any(|v| NumLike::#rule_fn(v)) };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike, V: NumLike> ValidateVec<(T,V)> for #name {
                fn validate_vec(&self, val: &[(T,V)]) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.0)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.0)) },
                        Target::Y => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.1)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.1)) },
                        Target::Both => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.0) && NumLike::#rule_fn(&v.1)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.0) || NumLike::#rule_fn(&v.1)) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }

            impl<T: NumLike + 'static> ValidateVec<Point2<T>> for #name {
                fn validate_vec(&self, val: &[Point2<T>]) -> OpmResult<()> {
                    let ok = match #target {
                        Target::X => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.x)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.x)) },
                        Target::Y => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.y)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.y)) },
                        Target::Both => if #mode_all { val.iter().all(|v| NumLike::#rule_fn(&v.x) && NumLike::#rule_fn(&v.y)) } else { val.iter().any(|v| NumLike::#rule_fn(&v.x) || NumLike::#rule_fn(&v.y)) },
                    };
                    if ok { Ok(()) } else { Err(OpossumError::Other(#msg_str.into())) }
                }
            }
        }
    };

    TokenStream::from(expanded)
}
