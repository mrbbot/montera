use crate::class::{FieldDescriptor, MethodDescriptor, ReturnDescriptor};
use nom::branch::alt;
use nom::bytes::complete::take_until1;
use nom::character::complete::char;
use nom::combinator::{map, value};
use nom::multi::many0;
use nom::sequence::tuple;
use nom::IResult;

// https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
pub fn field_descriptor_parser(input: &str) -> IResult<&str, FieldDescriptor> {
    alt((
        value(FieldDescriptor::Byte, char('B')),
        value(FieldDescriptor::Char, char('C')),
        value(FieldDescriptor::Double, char('D')),
        value(FieldDescriptor::Float, char('F')),
        value(FieldDescriptor::Int, char('I')),
        value(FieldDescriptor::Long, char('J')),
        value(FieldDescriptor::Short, char('S')),
        value(FieldDescriptor::Boolean, char('Z')),
        map(
            tuple((char('L'), take_until1(";"), char(';'))),
            |(_, class_name, _): (_, &str, _)| FieldDescriptor::Object(class_name.to_string()),
        ),
        map(
            tuple((char('['), field_descriptor_parser)),
            |(_, component_type)| FieldDescriptor::Array(Box::new(component_type)),
        ),
    ))(input)
}

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
pub fn return_descriptor_parser(input: &str) -> IResult<&str, ReturnDescriptor> {
    alt((
        value(ReturnDescriptor::Void, char('V')),
        map(field_descriptor_parser, |field_descriptor| {
            ReturnDescriptor::Field(field_descriptor)
        }),
    ))(input)
}

// https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
pub fn method_descriptor_parser(input: &str) -> IResult<&str, MethodDescriptor> {
    map(
        tuple((
            char('('),
            many0(field_descriptor_parser),
            char(')'),
            return_descriptor_parser,
        )),
        |(_, params, _, returns)| MethodDescriptor::new(params, returns),
    )(input)
}

#[cfg(test)]
mod tests {
    use crate::class::*;

    #[test]
    fn field_descriptor_parser_parses_base_types() {
        assert_eq!(
            field_descriptor_parser("B"),
            Ok(("", FieldDescriptor::Byte))
        );
        assert_eq!(
            field_descriptor_parser("C"),
            Ok(("", FieldDescriptor::Char))
        );
        assert_eq!(
            field_descriptor_parser("D"),
            Ok(("", FieldDescriptor::Double))
        );
        assert_eq!(
            field_descriptor_parser("F"),
            Ok(("", FieldDescriptor::Float))
        );
        assert_eq!(field_descriptor_parser("I"), Ok(("", FieldDescriptor::Int)));
        assert_eq!(
            field_descriptor_parser("J"),
            Ok(("", FieldDescriptor::Long))
        );
        assert_eq!(
            field_descriptor_parser("S"),
            Ok(("", FieldDescriptor::Short))
        );
        assert_eq!(
            field_descriptor_parser("Z"),
            Ok(("", FieldDescriptor::Boolean))
        );
    }

    #[test]
    fn field_descriptor_parser_parses_objects() {
        assert_eq!(
            field_descriptor_parser("Ljava/lang/Thread;"),
            Ok((
                "",
                FieldDescriptor::Object(String::from("java/lang/Thread"))
            ))
        );
        assert_eq!(
            field_descriptor_parser("Ljava/lang/Object;"),
            Ok((
                "",
                FieldDescriptor::Object(String::from("java/lang/Object"))
            ))
        );
    }

    #[test]
    fn field_descriptor_parser_parses_arrays() {
        assert_eq!(
            field_descriptor_parser("[D"),
            Ok((
                "",
                FieldDescriptor::Array(Box::new(FieldDescriptor::Double))
            ))
        );
        assert_eq!(
            field_descriptor_parser("[[[Ljava/lang/Object;"),
            Ok((
                "",
                FieldDescriptor::Array(Box::new(FieldDescriptor::Array(Box::new(
                    FieldDescriptor::Array(Box::new(FieldDescriptor::Object(String::from(
                        "java/lang/Object"
                    ))))
                ))))
            ))
        );
    }

    #[test]
    fn return_descriptor_parser_parses_void() {
        assert_eq!(
            return_descriptor_parser("V"),
            Ok(("", ReturnDescriptor::Void))
        );
    }

    #[test]
    fn return_descriptor_parser_parses_fields() {
        assert_eq!(
            return_descriptor_parser("B"),
            Ok(("", ReturnDescriptor::Field(FieldDescriptor::Byte)))
        );
        assert_eq!(
            return_descriptor_parser("Ljava/lang/Object;"),
            Ok((
                "",
                ReturnDescriptor::Field(FieldDescriptor::Object(String::from("java/lang/Object")))
            ))
        );
    }

    #[test]
    fn method_descriptor_parser_parses_methods() {
        assert_eq!(
            method_descriptor_parser("(IDLjava/lang/Thread;)Ljava/lang/Object;",),
            Ok((
                "",
                MethodDescriptor::new(
                    vec![
                        FieldDescriptor::Int,
                        FieldDescriptor::Double,
                        FieldDescriptor::Object(String::from("java/lang/Thread"))
                    ],
                    ReturnDescriptor::Field(FieldDescriptor::Object(String::from(
                        "java/lang/Object"
                    )))
                )
            ))
        );
        assert_eq!(
            method_descriptor_parser("()V"),
            Ok(("", MethodDescriptor::new(vec![], ReturnDescriptor::Void)))
        );
    }
}
