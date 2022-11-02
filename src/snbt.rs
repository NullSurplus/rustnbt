#![doc = r#"
This module is for parsing and generating SNBT. This module will only cover
Minecraft SNBT, and will not cover the Tag type extensions.

| Tag Type        | Syntax                                                                      |
|-----------------|-----------------------------------------------------------------------------|
|[Tag::Byte]      | `<number>b` or `<number>B`
|[Tag::Short]     | `<number>s` or `<number>S`
|[Tag::Int]       | `<integer_number>`
|[Tag::Long]      | `<number>l` or `<number>L`
|[Tag::Float]     | `<number>f` or `<number>F`
|[Tag::Double]    | `<decimal_number>`, `<number>d` or `<number>D`
|[Tag::ByteArray] | `[B; 0b, 1b, 2b]`
|[Tag::String]    | `"<text>"` or `'<text>'` or <identifier>
|[Tag::List]      | `[<tag>...]`
|[Tag::Compound]  | `{ <string:name> : <tag> }`
|[Tag::IntArray]  | `[I; 0, 1, 2 3]`
|[Tag::LongArray] | `[L; 0, 1, 2l, 3L]`

Note: Identifiers can include the following characters: [a-zA-Z0-9+-._].
For [Tag::List], the tag type for the list is determined by the type of the first tag.
"#]

use crate::*;
use crate::tag::*;
use chumsky::combinator::Repeated;
use chumsky::prelude::*;
use chumsky::primitive::{
    Container,
    OneOf,
    NoneOf,
};
use chumsky::Error;
use chumsky::text::Character;
use core::panic;
use std::collections::HashSet;
use std::result;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("Found invalid token(s).")]
    TokenizeError(Vec<Simple<char>>),
    #[error("Failed to parse SNBT.")]
    ParseFailure(Vec<Simple<Token>>),
}

// problem: I need to parse the suffix of numeric literals:
// Byte     b or B
// Short    s or S
// Int      <none>
// Long     l or L
// Float    f or F
// Double   d or D or <none>
// Since these are case-insensitive

fn is_ident_char(c: &char) -> bool {
    c.is_ascii_alphanumeric() || ['_','-','+','.'].contains(c)
}

fn identifier<E: chumsky::Error<char>>() -> impl Parser<char, String, Error = E> {
    filter::<char,_,E>(is_ident_char)
        .repeated()
        .at_least(1)
        .collect::<String>()
}

fn strcmp(ignore_case: bool, lhs: &str, rhs: &str, ) -> bool {
    if lhs.len() != rhs.len() {
        return false;
    }
    if ignore_case {
        lhs.to_lowercase() == rhs.to_lowercase()
    } else {
        lhs == rhs
    }
}

fn keyword<S: AsRef<str>>(word: S, ignore_case: bool) -> impl Parser<char, (), Error = Simple<char>> {
    identifier()
        .try_map(move |text, span| {
            if strcmp(ignore_case, word.as_ref(), &text) {
                Ok(())
            } else {
                Err(Simple::custom(span, format!("Expected keyword: {}, found {}", word.as_ref(), text)))
            }
        })
}

fn no_case<C: Container<char>>(chars: C) -> HashSet<char> {
    let mut set: HashSet<char> = HashSet::new();
    chars.get_iter().for_each(|c| {
        set.insert(c);
        if c.is_lowercase() {
            set.extend(c.to_uppercase());
        } else {
            set.extend(c.to_lowercase());
        }
    });
    set
}

fn one_of_nc<C: Container<char>, E: Error<char>>(chars: C) -> OneOf<char,HashSet<char>,E> {
    one_of(no_case(chars))
}

fn none_of_nc<C: Container<char>, E: Error<char>>(chars: C) -> NoneOf<char,HashSet<char>,E> {
    none_of(no_case(chars))
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone, Hash, Debug)]
pub enum ArrayType {
    Byte = 'B' as isize,
    Int = 'I' as isize,
    Long = 'L' as isize,
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone, Hash, Debug)]
pub enum IntegerType {
    Byte,
    Int,
    Short,
    Long,
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone, Hash, Debug)]
pub enum DecimalType {
    Float,
    Double,
}

impl From<Option<char>> for DecimalType {
    /// Must be either 'D' or 'F' (case-insensitive) or else it will panic.
    fn from(ch: Option<char>) -> Self {
        match ch {
            Some(ch) => match ch {
                'f' => DecimalType::Float,
                'd' => DecimalType::Double,
                _ => panic!("Must be either 'f' or 'd'."),
            },
            None => DecimalType::Double,
            _ => panic!("Invalid DecimalType"),
        }
    }
}

impl From<Option<char>> for IntegerType {
    /// Must be one of 'B', 'S', or 'L' (case-insensitive) or else it will panic.
    fn from(ch: Option<char>) -> Self {
        match ch {
            Some(ch) => match ch.to_ascii_lowercase() {
                'b' => IntegerType::Byte,
                's' => IntegerType::Short,
                'l' => IntegerType::Long,
                _ => panic!("Impossible! I think..."),
            },
            None => IntegerType::Int,
            _ => panic!(),
        }
    }
}

impl From<char> for ArrayType {
    fn from(ch: char) -> Self {
        match ch {
            'B' => ArrayType::Byte,
            'I' => ArrayType::Int,
            'L' => ArrayType::Long,
            _ => panic!("Expected either 'B', 'I', or 'L'."),
        }
    }
}

#[derive(PartialEq, Eq,PartialOrd, Ord, Clone, Hash, Debug)]
pub enum Token {
    Comma,
    Colon,
    ArrayStart(ArrayType),
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Boolean(bool),
    Integer(String, IntegerType),
    Decimal(String, DecimalType),
    Identifier(String),
    StringLiteral(String),
}

macro_rules! token_api {
    ($($name:ident => $block:block)+) => {
        impl Token {
            $(
                pub fn $name() -> impl Parser<char, Token, Error = Simple<char>>
                $block
            )+

            pub fn parse<S: AsRef<str>>(source: S) -> Result<Vec<Token>, Vec<Simple<char>>> {
                choice((
                    $(
                        Self::$name(),
                    )+
                ))
                .padded()
                .repeated().at_least(1)
                .then_ignore(end())
                .collect::<Vec<Token>>()
                .parse(source.as_ref())
            }
        }
    };
}
// This macro helps to create the lexer.
// 
token_api!{
    comma => { just(',').to(Token::Comma).labelled("Comma") }
    colon => { just(':').to(Token::Colon).labelled("Colon") }
    array_start => {
        just('[')
            .ignore_then(
                choice((
                    keyword("b", true).to(ArrayType::Byte),
                    keyword("i", true).to(ArrayType::Int),
                    keyword("l", true).to(ArrayType::Long),
                ))
            )
            .then_ignore(just(';'))
            .map(Token::ArrayStart)
            .labelled("Array Start")
    }
    open_bracket => { just('[').to(Token::OpenBracket).labelled("Open Bracket") }
    close_bracket => { just(']').to(Token::CloseBracket).labelled("Close Bracket") }
    open_brace => { just('{').to(Token::OpenBrace).labelled("Open Brace") }
    close_brace => { just('}').to(Token::CloseBrace).labelled("Close Brace") }
    boolean => {
        choice((
            text::keyword("true").to(Token::Boolean(true)),
            text::keyword("false").to(Token::Boolean(false)),
        ))
        .labelled("Boolean")
    }
    // If I want, I can add binary and hex literals.
    integer => {
        just::<char, _, Simple<char>>('-')
            .or_not()
            .chain::<char, _, _>(text::int(10))
            .collect::<String>()
            .then(
                choice((
                    keyword("b", true).to(IntegerType::Byte),
                    keyword("s", true).to(IntegerType::Short),
                    keyword("l", true).to(IntegerType::Long),
                ))
                .or_not()
                .map(|opt| opt.unwrap_or(IntegerType::Int))
            )
            .then_ignore(choice((
                filter(|c: &char| {
                    !c.is_alphanumeric() && !['_', '+','-','.'].contains(c)
                }),
                end().to('\0')
            )).rewind())
            .map(|(int_text, int_type)| Token::Integer(int_text, int_type))
            .labelled("Integer")
    }
    decimal => {
        just::<char, _, Simple<char>>('-').or_not()
            .chain::<char,_,_>(
                choice((
                    text::int(10)
                        .chain::<char,_,_>(just('.'))
                        .chain::<char,_,_>(text::digits(10))
                        .collect::<String>(),
                    text::int(10)
                        .then_ignore(
                            choice((
                                keyword("d", true),
                                keyword("f", true),
                            )).rewind()
                        ),
                ))
            )
            .collect::<String>()
            .then(
                choice((
                    keyword("d", true).to(DecimalType::Double),
                    keyword("f", true).to(DecimalType::Float),
                ))
                .or_not()
                .map(|opt| opt.unwrap_or(DecimalType::Double))
            )
            .then_ignore(choice((
                filter(|c: &char| {
                    !c.is_alphanumeric() && !['_', '+','-','.'].contains(c)
                }),
                end().to('\0')
            )).rewind())
            .map(|(dec_str, dec_type)| Token::Decimal(dec_str, dec_type))
            .labelled("Decimal")
    }
    identifier => {
        choice((
            filter(char::is_ascii_alphanumeric),
            one_of("+-_.")
        ))
        .repeated().at_least(1)
        .collect::<String>()
        .map(Token::Identifier)
        .labelled("Identifier")
    }
    string_literal => {
        let escape = just::<_,_,Simple<char>>('\\').ignore_then(
            just('\\')
                .or(just('/'))
                .or(just('"'))
                .or(just('\''))
                .or(just('b').to('\x08'))
                .or(just('f').to('\x0C'))
                .or(just('n').to('\n'))
                .or(just('r').to('\r'))
                .or(just('t').to('\t'))
        );
        Token::identifier().or(
            choice::<_,Simple<char>>((
                just('"')
                    .ignore_then(
                        none_of("\\\"").or(escape.clone()).repeated()
                    )
                    .then_ignore(just('"'))
                    .collect::<String>(),
                just('\'')
                    .ignore_then(
                        none_of("\\'").or(escape.clone()).repeated()
                    )
                    .then_ignore(just('\''))
                    .collect::<String>(),
            )).map(Token::StringLiteral))
            .labelled("String Literal")
    }
}

fn parser() -> impl Parser<Token, Tag, Error = Simple<Token>> {
    macro_rules! num_parsers {
        ($(let $name:ident = Token::$token_type:ident($subtype:path) => $type:ty;)+) => {
            $(
                let $name = filter::<Token,_,Simple<Token>>(|token| matches!(token, Token::$token_type(_, $subtype)))
                    .try_map(|token, span| {
                        match token {
                            Token::$token_type(digits, $subtype) => {
                                digits.parse::<$type>().map_err(|_| Simple::custom(span, concat!("Failed to parse.")))
                            },
                            _ => Err(Simple::custom(span, "Invalid token.")),
                        }
                    });
            )+
        };
    }
    num_parsers!{
        let byte = Token::Integer(IntegerType::Byte) => i8;
        let short = Token::Integer(IntegerType::Short) => i16;
        let int = Token::Integer(IntegerType::Int) => i32;
        let long = Token::Integer(IntegerType::Long) => i64;
        let float = Token::Decimal(DecimalType::Float) => f32;
        let double = Token::Decimal(DecimalType::Double) => f64;
    };
    let byte = byte.or(
        choice((
            filter(|token| matches!(token, Token::Boolean(true))).to(1i8),
            filter(|token| matches!(token, Token::Boolean(false))).to(0i8),
        ))
    );
    macro_rules! array_parsers {
        ($(let $name:ident = [$type:ident; $item:expr];)+) => {
            $(
                let $name = ($item)
                    .separated_by(just(Token::Comma))
                    .delimited_by(just(Token::ArrayStart(ArrayType::$type)), just(Token::CloseBracket));
            )+
        };
    }
    array_parsers!{
        let bytearray = [Byte; byte.clone()];
        let intarray = [Int; int.clone()];
        let longarray = [Long; long.clone()];
    }
    let byte = byte.or(
        filter::<Token,_,Simple<Token>>(|token| matches!(token, Token::Boolean(_)))
            .map(|token| match token {
                Token::Boolean(true) => 1i8,
                _ => 0i8,
            })
    );
    // converts Token::StringLiteral and Token::Identifier into String.
    // This is because these tokens may mean different things in different contexts.
    let string = filter::<Token,_,Simple<Token>>(|token| matches!(token, Token::StringLiteral(_) | Token::Identifier(_)))
        .map(|token| match token {
            Token::StringLiteral(data) => data,
            Token::Identifier(data) => data,
            _ => panic!("Impossible state.")
        });
    let mut list = Recursive::declare();
    let mut compound = Recursive::declare();
    macro_rules! list_maker {
        ($([$pattern:expr]),+) => {
            choice::<_,Simple<Token>>((
                $(
                    ($pattern)
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .delimited_by(just(Token::OpenBracket), just(Token::CloseBracket))
                        .map(ListTag::from),
                )+
            ))
        };
    }
    list.define(
        list_maker!{
            [byte.clone()],
            [short.clone()],
            [int.clone()],
            [long.clone()],
            [float.clone()],
            [double.clone()],
            [bytearray.clone()],
            [string.clone()],
            [list.clone()],
            [compound.clone()],
            [intarray.clone()],
            [longarray.clone()]
        }
    );

    compound.define(
        string.clone()
            .then_ignore(just(Token::Colon))
            .then(choice((
                compound.clone().map(Tag::Compound),
                list.clone().map(Tag::List),
                byte.clone().map(Tag::Byte),
                short.clone().map(Tag::Short),
                int.clone().map(Tag::Int),
                long.clone().map(Tag::Long),
                float.clone().map(Tag::Float),
                double.clone().map(Tag::Double),
                bytearray.clone().map(Tag::ByteArray),
                intarray.clone().map(Tag::IntArray),
                longarray.clone().map(Tag::LongArray),
                string.clone().map(Tag::String)
            )))
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .delimited_by(just(Token::OpenBrace), just(Token::CloseBrace))
            .map(crate::Map::from_iter)
    );
    choice((
        compound.clone().map(Tag::Compound),
        list.clone().map(Tag::List),
        byte.clone().map(Tag::Byte),
        short.clone().map(Tag::Short),
        int.clone().map(Tag::Int),
        long.clone().map(Tag::Long),
        float.clone().map(Tag::Float),
        double.clone().map(Tag::Double),
        bytearray.clone().map(Tag::ByteArray),
        intarray.clone().map(Tag::IntArray),
        longarray.clone().map(Tag::LongArray),
        string.clone().map(Tag::String)
    ))
}

#[test]
fn parsetest() {
    let snbt = r#"
    {
        byte1 : 0b,
        byte2 : -10b,
        byte3 : 127b,
        short : 69s,
        int : 420,
        long : 69420,
        float : 3f,
        float2 : 3.14f,
        double : 4d,
        double2 : 4.5d,
        double3 : 5.1,
        bytearray : [B; true, false, 5b],
        intarray : [I; 3, 5, 1],
        longarray : [L; 3l, 4l, 5l],
        list : [4b, 3b, 2b],
        compound : {
            "test" : "The quick brown fox jumps over the lazy dog."
        }
    }
    "#;
    if let Ok(Tag::Compound(result)) = parse(snbt) {
        macro_rules! check_keys {
            ($($key:literal)+) => {
                $(
                    assert!(result.contains_key($key));
                )+
            };
        }
        check_keys!{
            "byte1"
            "byte2"
            "byte3"
            "short"
            "int"
            "long"
            "float"
            "float2"
            "double"
            "double3"
            "bytearray"
            "intarray"
            "longarray"
            "list"
            "compound"
        }
    } else {
        panic!();
    }
}

/// Attempt to parse Minecraft SNBT format.
/// ### Example
/// ```
/// # use rustnbt::{*,tag::*,io::*,snbt::*};
/// let snbt = r#"
/// {
///     byte1 : 0b,
///     byte2 : -10b,
///     byte3 : 127b,
///     short : 69s,
///     int : 420,
///     long : 69420,
///     float : 3f,
///     float2 : 3.14f,
///     double : 4d,
///     double2 : 4.5d,
///     double3 : 5.1,
///     bytearray : [B; true, false, 5b],
///     intarray : [I; 3, 5, 1],
///     longarray : [L; 3l, 4l, 5l],
///     list : [4b, 3b, 2b],
///     compound : {
///         "test" : "The quick brown fox jumps over the lazy dog."
///     }
/// }
/// "#;
/// if let Ok(Tag::Compound(result)) = parse(snbt) {
///     assert!(result.contains_key(&"double".to_string()));
/// } else {
///     panic!();
/// }
/// ```
pub fn parse<S: AsRef<str>>(source: S) -> Result<Tag, ParseError> {
    match Token::parse(source) {
        Ok(tokens) => {
            match parser().parse(tokens) {
                Ok(tag) => Ok(tag),
                Err(errors) => Err(ParseError::ParseFailure(errors)),
            }
        },
        Err(errors) => Err(ParseError::TokenizeError(errors)),
    }
}

#[cfg(test)]
fn test_parse<S: AsRef<str>>(source: S) {
    match parse(source) {
        Ok(result) => {
            println!("{}", result);
        }
        Err(err) => {
            eprintln!("{:#?}", err);
        }
    }
}

#[test]
fn foo() {
    // [warning]: DELETE ME!
    test_parse(r#"
{
    byte1 : 0b,
    byte2 : -10b,
    byte3 : 127b,
    short : 69s,
    int : 420,
    long : 69420,
    float : 3f,
    float2 : 3.14f,
    double : 4d,
    double2 : 4.5d,
    double3 : 5.1,
    bytearray : [B; true, false, 5b],
    intarray : [I; 3, 5, 1],
    longarray : [L; 3l, 4l, 5l],
    list : [4b, 3b, 2b],
    compound : {
        "test" : "The quick brown fox jumps over the lazy dog."
    }
}
    "#);
}