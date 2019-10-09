// ccommon - a cache common library.
// Copyright (C) 2019 Twitter, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::parse::*;
use syn::parse_quote;
use syn::punctuated::*;
use syn::spanned::Spanned;
use syn::*;

use std::mem;

pub struct AttrOption<T> {
    pub name: Ident,
    pub eq: Token![=],
    pub val: T,
}

pub type EqOption = AttrOption<Lit>;
pub type ExprOption = AttrOption<Expr>;

pub struct MetricAttr {
    pub desc: EqOption,
    pub name: Option<EqOption>,
}

pub struct OptionAttr {
    pub desc: EqOption,
    pub name: Option<EqOption>,
    pub default: Option<ExprOption>,
}

enum StrOrExpr {
    Str(LitStr),
    Expr(Expr),
}

impl Parse for StrOrExpr {
    fn parse(buf: &ParseBuffer) -> Result<Self> {
        if buf.fork().parse::<LitStr>().is_ok() {
            return Ok(StrOrExpr::Str(buf.parse()?));
        }

        buf.parse().map(StrOrExpr::Expr)
    }
}

impl AttrOption<StrOrExpr> {
    pub fn as_lit(self) -> Result<EqOption> {
        match self.val {
            StrOrExpr::Str(s) => Ok(EqOption {
                name: self.name,
                eq: self.eq,
                val: Lit::Str(s),
            }),
            StrOrExpr::Expr(e) => Err(Error::new(
                e.span(),
                "Found expression, expected a literal string",
            )),
        }
    }

    pub fn as_expr(self) -> ExprOption {
        let expr = match self.val {
            StrOrExpr::Str(s) => parse_quote!(#s),
            StrOrExpr::Expr(e) => e,
        };

        ExprOption {
            name: self.name,
            eq: self.eq,
            val: expr,
        }
    }
}

impl<T: Parse> Parse for AttrOption<T> {
    fn parse(buf: &ParseBuffer) -> Result<Self> {
        Ok(Self {
            name: buf.parse()?,
            eq: buf.parse()?,
            val: buf.parse()?,
        })
    }
}

impl Parse for MetricAttr {
    fn parse(buf: &ParseBuffer) -> Result<Self> {
        let seq: Punctuated<EqOption, Token![,]> = Punctuated::parse_terminated(buf)?;

        let mut desc = None;
        let mut name = None;

        for opt in seq {
            let span = opt.name.span();
            let param = opt.name.to_string();
            let seen = match &*param {
                "desc" => mem::replace(&mut desc, Some(opt)).is_some(),
                "name" => mem::replace(&mut name, Some(opt)).is_some(),
                _ => {
                    return Err(Error::new(
                        opt.name.span(),
                        format!("Unknown option `{}`", opt.name),
                    ))
                }
            };

            if seen {
                return Err(Error::new(
                    span,
                    format!("`{}` may only be specified once", param),
                ));
            }
        }

        let desc = match desc {
            Some(x) => x,
            None => return Err(buf.error("Expected a `desc` parameter here")),
        };

        Ok(Self { desc, name })
    }
}

impl Parse for OptionAttr {
    fn parse(buf: &ParseBuffer) -> Result<Self> {
        let seq: Punctuated<AttrOption<StrOrExpr>, Token![,]> = Punctuated::parse_terminated(buf)?;

        let mut desc = None;
        let mut name = None;
        let mut default = None;

        for val in seq {
            let span = val.span();
            let param = val.name.to_string();
            let seen = match &*param {
                "desc" => mem::replace(&mut desc, Some(val.as_lit()?)).is_some(),
                "name" => mem::replace(&mut name, Some(val.as_lit()?)).is_some(),
                "default" => mem::replace(&mut default, Some(val.as_expr())).is_some(),
                _ => return Err(Error::new(span, format!("Unknown option `{}`", param))),
            };

            if seen {
                return Err(Error::new(
                    span,
                    format!("`{}` may only be specified once", param),
                ));
            }
        }

        let desc = match desc {
            Some(x) => x,
            None => return Err(buf.error("Expected a `desc` parameter here")),
        };

        Ok(Self {
            desc,
            name,
            default,
        })
    }
}

impl<T: ToTokens> ToTokens for AttrOption<T> {
    fn to_tokens(&self, stream: &mut TokenStream) {
        self.name.to_tokens(stream);
        self.eq.to_tokens(stream);
        self.val.to_tokens(stream);
    }
}

impl ToTokens for StrOrExpr {
    fn to_tokens(&self, stream: &mut TokenStream) {
        match self {
            Self::Str(s) => s.to_tokens(stream),
            Self::Expr(e) => e.to_tokens(stream),
        }
    }
}
