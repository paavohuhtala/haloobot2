use std::borrow::Cow;

use nom::{
    self,
    bytes::complete::{escaped_transform, tag, take_while1},
    character::complete::{none_of, one_of},
    combinator::{all_consuming, map},
    multi::{many0, many1, separated_list1},
    sequence::delimited,
    IResult,
};

pub fn parse_arguments(input: &str) -> IResult<&str, Vec<Cow<str>>> {
    let parse_quoted_argument = map(
        delimited(
            tag("\""),
            escaped_transform(none_of("\\\""), '\\', one_of("\"\\")),
            tag("\""),
        ),
        |s: String| Cow::Owned(s),
    );
    let parse_unquoted_argument = map(take_while1(|c| c != ' ' && c != '"'), |s: &str| {
        Cow::Borrowed(s)
    });

    let parse_argument_list = separated_list1(
        many1(tag(" ")),
        nom::branch::alt((parse_quoted_argument, parse_unquoted_argument)),
    );

    let mut parse_arguments = all_consuming(delimited(
        many0(tag(" ")),
        parse_argument_list,
        many0(tag(" ")),
    ));

    parse_arguments(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_quoted() {
        let input = "\"hello\" \"world\"";
        let expected = vec![Cow::Borrowed("hello"), Cow::Borrowed("world")];
        let actual = parse_arguments(input).unwrap().1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_quoted_with_space() {
        let input = "\"hello world\"";
        let expected = vec![Cow::Borrowed("hello world")];
        let actual = parse_arguments(input).unwrap().1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_unquoted() {
        let input = "hello world";
        let expected = vec![Cow::Borrowed("hello"), Cow::Borrowed("world")];
        let actual = parse_arguments(input).unwrap().1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_mixed() {
        let input = "hello \"world\"";
        let expected = vec![Cow::Borrowed("hello"), Cow::Borrowed("world")];
        let actual = parse_arguments(input).unwrap().1;
        assert_eq!(actual, expected);
    }

    #[test]
    fn parse_escaped() {
        let input = r#""hello \"world\"""#;
        let expected: Vec<Cow<str>> = vec![Cow::Owned("hello \"world\"".to_string())];
        let actual = parse_arguments(input).unwrap().1;
        assert_eq!(actual, expected);
    }
}
