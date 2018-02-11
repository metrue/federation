use combine::{parser, ParseResult, Parser};
use combine::easy::{Error, Errors};
use combine::error::StreamError;
use combine::combinator::{many, many1, eof, optional, position, choice};
use combine::combinator::{sep_by1};

use tokenizer::{Kind as T, Token, TokenStream};
use helpers::{punct, ident, kind, name};
use common::{directives, string, default_value, parse_type};
use schema::error::{SchemaParseError};
use schema::ast::*;


pub fn schema<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<SchemaDefinition, TokenStream<'a>>
{
    (
        position().skip(ident("schema")),
        parser(directives),
        punct("{")
            .with(many((
                kind(T::Name).skip(punct(":")),
                name(),
            )))
            .skip(punct("}")),
    )
    .flat_map(|(position, directives, operations): (_, _, Vec<(Token, _)>)| {
        let mut query = None;
        let mut mutation = None;
        let mut subscription = None;
        let mut err = Errors::empty(position);
        for (oper, type_name) in operations {
            match oper.value {
                "query" if query.is_some() => {
                    err.add_error(Error::unexpected_static_message(
                        "duplicate `query` operation"));
                }
                "query" => {
                    query = Some(type_name);
                }
                "mutation" if mutation.is_some() => {
                    err.add_error(Error::unexpected_static_message(
                        "duplicate `mutation` operation"));
                }
                "mutation" => {
                    mutation = Some(type_name);
                }
                "subscription" if subscription.is_some() => {
                    err.add_error(Error::unexpected_static_message(
                        "duplicate `subscription` operation"));
                }
                "subscription" => {
                    subscription = Some(type_name);
                }
                _ => {
                    err.add_error(Error::unexpected_token(oper));
                    err.add_error(
                        Error::expected_static_message("query"));
                    err.add_error(
                        Error::expected_static_message("mutation"));
                    err.add_error(
                        Error::expected_static_message("subscription"));
                }
            }
        }
        if !err.errors.is_empty() {
            return Err(err);
        }
        Ok(SchemaDefinition {
            position, directives, query, mutation, subscription,
        })
    })
    .parse_stream(input)
}

pub fn scalar_type<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<ScalarType, TokenStream<'a>>
{
    (
        position(),
        ident("scalar").with(name()),
        parser(directives),
    )
        .map(|(position, name, directives)| {
            ScalarType { position, description: None, name, directives }
        })
        .parse_stream(input)
}

pub fn scalar_type_extension<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<ScalarTypeExtension, TokenStream<'a>>
{
    (
        position(),
        ident("scalar").with(name()),
        parser(directives),
    )
    .flat_map(|(position, name, directives)| {
        if directives.is_empty() {
            let mut e = Errors::empty(position);
            e.add_error(Error::expected_static_message(
                "Scalar type extension should contain at least \
                 one directive."));
            return Err(e);
        }
        Ok(ScalarTypeExtension { position, name, directives })
    })
    .parse_stream(input)
}

pub fn implements_interfaces<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Vec<NamedType>, TokenStream<'a>>
{
    optional(
        ident("implements")
        .skip(optional(punct("&")))
        .with(sep_by1(name(), punct("&")))
    )
        .map(|opt| opt.unwrap_or_else(Vec::new))
        .parse_stream(input)
}

pub fn input_value<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<InputValue, TokenStream<'a>>
{
    (
        position(),
        optional(parser(string)),
        name(),
        punct(":").with(parser(parse_type)),
        optional(punct("=").with(parser(default_value))),
        parser(directives),
    )
    .map(|(position, description, name, value_type, default_value, directives)|
    {
        InputValue {
            position, description, name, value_type, default_value, directives,
        }
    })
    .parse_stream(input)
}

pub fn arguments_definition<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Vec<InputValue>, TokenStream<'a>>
{
    optional(punct("(").with(many1(parser(input_value))).skip(punct(")")))
    .map(|v| v.unwrap_or_else(Vec::new))
    .parse_stream(input)
}

pub fn field<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Field, TokenStream<'a>>
{
    (
        position(),
        optional(parser(string)),
        name(),
        parser(arguments_definition),
        punct(":").with(parser(parse_type)),
        parser(directives),
    )
    .map(|(position, description, name, arguments, field_type, directives)| {
        Field {
            position, description, name, arguments, field_type, directives
        }
    })
    .parse_stream(input)
}

pub fn fields<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Vec<Field>, TokenStream<'a>>
{
    optional(punct("{").with(many1(parser(field))).skip(punct("}")))
    .map(|v| v.unwrap_or_else(Vec::new))
    .parse_stream(input)
}


pub fn object_type<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<ObjectType, TokenStream<'a>>
{
    (
        position(),
        ident("type").with(name()),
        parser(implements_interfaces),
        parser(directives),
        parser(fields),
    )
        .map(|(position, name, interfaces, directives, fields)| {
            ObjectType {
                position, name, directives, fields,
                implements_interfaces: interfaces,
                description: None,  // is filled in type_definition
            }
        })
        .parse_stream(input)
}

pub fn object_type_extension<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<ObjectTypeExtension, TokenStream<'a>>
{
    (
        position(),
        ident("type").with(name()),
        parser(implements_interfaces),
        parser(directives),
        parser(fields),
    )
        .flat_map(|(position, name, interfaces, directives, fields)| {
            if interfaces.is_empty() && directives.is_empty() &&
                fields.is_empty()
            {
                let mut e = Errors::empty(position);
                e.add_error(Error::expected_static_message(
                    "Object type extension should contain at least \
                     one interface, directive or field."));
                return Err(e);
            }
            Ok(ObjectTypeExtension {
                position, name, directives, fields,
                implements_interfaces: interfaces,
            })
        })
        .parse_stream(input)
}

pub fn interface_type<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<InterfaceType, TokenStream<'a>>
{
    (
        position(),
        ident("interface").with(name()),
        parser(directives),
        parser(fields),
    )
        .map(|(position, name, directives, fields)| {
            InterfaceType {
                position, name, directives, fields,
                description: None,  // is filled in type_definition
            }
        })
        .parse_stream(input)
}

pub fn interface_type_extension<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<InterfaceTypeExtension, TokenStream<'a>>
{
    (
        position(),
        ident("interface").with(name()),
        parser(directives),
        parser(fields),
    )
        .flat_map(|(position, name, directives, fields)| {
            if directives.is_empty() && fields.is_empty() {
                let mut e = Errors::empty(position);
                e.add_error(Error::expected_static_message(
                    "Interface type extension should contain at least \
                     one directive or field."));
                return Err(e);
            }
            Ok(InterfaceTypeExtension {
                position, name, directives, fields,
            })
        })
        .parse_stream(input)
}

pub fn union_members<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Vec<NamedType>, TokenStream<'a>>
{
    optional(punct("|"))
    .with(sep_by1(name(), punct("|")))
    .parse_stream(input)
}

pub fn union_type<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<UnionType, TokenStream<'a>>
{
    (
        position(),
        ident("union").with(name()),
        parser(directives),
        optional(punct("=").with(parser(union_members))),
    )
    .map(|(position, name, directives, types)| {
        UnionType {
            position, name, directives,
            types: types.unwrap_or_else(Vec::new),
            description: None,  // is filled in type_definition
        }
    })
    .parse_stream(input)
}

pub fn union_type_extension<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<UnionTypeExtension, TokenStream<'a>>
{
    (
        position(),
        ident("union").with(name()),
        parser(directives),
        optional(punct("=").with(parser(union_members))),
    )
    .flat_map(|(position, name, directives, types)| {
        if directives.is_empty() && types.is_none() {
            let mut e = Errors::empty(position);
            e.add_error(Error::expected_static_message(
                "Union type extension should contain at least \
                 one directive or type."));
            return Err(e);
        }
        Ok(UnionTypeExtension {
            position, name, directives,
            types: types.unwrap_or_else(Vec::new),
        })
    })
    .parse_stream(input)
}

pub fn type_definition<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<TypeDefinition, TokenStream<'a>>
{
    (
        optional(parser(string)),
        choice((
            parser(scalar_type).map(TypeDefinition::Scalar),
            parser(object_type).map(TypeDefinition::Object),
            parser(interface_type).map(TypeDefinition::Interface),
            parser(union_type).map(TypeDefinition::Union),
        )),
    )
        // We can't set description inside type definition parser, because
        // that means parser will need to backtrace, and that in turn
        // means that error reporting is bad (along with performance)
        .map(|(descr, mut def)| {
            use schema::ast::TypeDefinition::*;
            match def {
                Scalar(ref mut s) => s.description = descr,
                Object(ref mut o) => o.description = descr,
                Interface(ref mut i) => i.description = descr,
                Union(ref mut u) => u.description = descr,
                Enum(ref mut e) => e.description = descr,
                InputObject(ref mut o) => o.description = descr,
            }
            def
        })
        .parse_stream(input)
}

pub fn type_extension<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<TypeExtension, TokenStream<'a>>
{
    ident("extend")
    .with(choice((
        parser(scalar_type_extension).map(TypeExtension::Scalar),
        parser(object_type_extension).map(TypeExtension::Object),
        parser(interface_type_extension).map(TypeExtension::Interface),
        parser(union_type_extension).map(TypeExtension::Union),
    )))
    .parse_stream(input)
}


pub fn definition<'a>(input: &mut TokenStream<'a>)
    -> ParseResult<Definition, TokenStream<'a>>
{
    choice((
        parser(schema).map(Definition::SchemaDefinition),
        parser(type_definition).map(Definition::TypeDefinition),
        parser(type_extension).map(Definition::TypeExtension),
    )).parse_stream(input)
}

/// Parses a piece of schema language and returns an AST
pub fn parse_schema(s: &str) -> Result<Document, SchemaParseError> {
    let mut tokens = TokenStream::new(s);
    let (doc, _) = many1(parser(definition))
        .map(|d| Document { definitions: d })
        .skip(eof())
        .parse_stream(&mut tokens)
        .map_err(|e| e.into_inner().error)?;

    Ok(doc)
}


#[cfg(test)]
mod test {
    use position::Pos;
    use schema::grammar::*;
    use super::parse_schema;

    fn ast(s: &str) -> Document {
        parse_schema(s).unwrap()
    }

    #[test]
    fn one_field() {
        assert_eq!(ast("schema { query: Query }"), Document {
            definitions: vec![
                Definition::SchemaDefinition(
                    SchemaDefinition {
                        position: Pos { line: 1, column: 1 },
                        directives: vec![],
                        query: Some("Query".into()),
                        mutation: None,
                        subscription: None
                    }
                )
            ],
        });
    }
}
