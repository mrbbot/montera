//! Parsers for field and method descriptors representing field types and method parameter/return
//! types respectively.
//!
//! Parsers are implemented using parser combinators, higher-order functions composing smaller
//! parsers into larger ones. Each combinator returns an output and the remaining input, or an error
//! if parsing failed. The [`nom`] crate defines a selection of [combinators].
//!
//! [combinators]: https://github.com/Geal/nom/blob/main/doc/choosing_a_combinator.md
//!
//! # Grammar
//!
//! Adapted from sections [4.3.2] and [4.3.3] of the Java Virtual Machine Specification:
//!
//! ```text
//! FieldDescriptor  ::= BaseType | ObjectType | ArrayType
//! BaseType         ::= 'B' | 'C' | 'D' | 'F' | 'I' | 'J' | 'S' | 'Z'
//! ObjectType       ::= 'L' ClassName ';'
//! ArrayType        ::= '[' FieldDescriptor
//! MethodDescriptor ::= '(' FieldDescriptor* ')' ReturnDescriptor
//! ReturnDescriptor ::= FieldDescriptor | 'V'
//! ```
//!
//! [4.3.2]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
//! [4.3.3]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
use crate::class::{FieldDescriptor, MethodDescriptor, ReturnDescriptor};
use nom::branch::alt;
use nom::bytes::complete::take_until1;
use nom::character::complete::char;
use nom::combinator::{map, value};
use nom::multi::many0;
use nom::sequence::tuple;
use nom::IResult;

/// Parses a field descriptor according to the grammar defined in section [4.3.2] of the Java
/// Virtual Machine Specification.
///
/// [4.3.2]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.2
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

/// Parses a return descriptor according to the grammar defined in section [4.3.3] of the Java
/// Virtual Machine Specification.
///
/// [4.3.3]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
pub fn return_descriptor_parser(input: &str) -> IResult<&str, ReturnDescriptor> {
    alt((
        value(ReturnDescriptor::Void, char('V')),
        map(field_descriptor_parser, |field_descriptor| {
            ReturnDescriptor::Field(field_descriptor)
        }),
    ))(input)
}

/// Parses a method descriptor according to the grammar defined in section [4.3.3] of the Java
/// Virtual Machine Specification.
///
/// [4.3.3]: https://docs.oracle.com/javase/specs/jvms/se7/html/jvms-4.html#jvms-4.3.3
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
    fn field_descriptor_parser_parses_base_types() -> anyhow::Result<()> {
        let (rest, d) = field_descriptor_parser("B")?;
        assert_eq!(format!("{}", d), "B");
        assert_eq!((rest, d), ("", FieldDescriptor::Byte));

        let (rest, d) = field_descriptor_parser("C")?;
        assert_eq!(format!("{}", d), "C");
        assert_eq!((rest, d), ("", FieldDescriptor::Char));

        let (rest, d) = field_descriptor_parser("D")?;
        assert_eq!(format!("{}", d), "D");
        assert_eq!((rest, d), ("", FieldDescriptor::Double));

        let (rest, d) = field_descriptor_parser("F")?;
        assert_eq!(format!("{}", d), "F");
        assert_eq!((rest, d), ("", FieldDescriptor::Float));

        let (rest, d) = field_descriptor_parser("I")?;
        assert_eq!(format!("{}", d), "I");
        assert_eq!((rest, d), ("", FieldDescriptor::Int));

        let (rest, d) = field_descriptor_parser("J")?;
        assert_eq!(format!("{}", d), "J");
        assert_eq!((rest, d), ("", FieldDescriptor::Long));

        let (rest, d) = field_descriptor_parser("S")?;
        assert_eq!(format!("{}", d), "S");
        assert_eq!((rest, d), ("", FieldDescriptor::Short));

        let (rest, d) = field_descriptor_parser("Z")?;
        assert_eq!(format!("{}", d), "Z");
        assert_eq!((rest, d), ("", FieldDescriptor::Boolean));

        Ok(())
    }

    #[test]
    fn field_descriptor_parser_parses_objects() -> anyhow::Result<()> {
        let (rest, d) = field_descriptor_parser("Ljava/lang/Thread;")?;
        assert_eq!(format!("{}", d), "Ljava/lang/Thread;");
        assert_eq!(rest, "");
        assert_eq!(d, FieldDescriptor::Object(String::from("java/lang/Thread")));

        let (rest, d) = field_descriptor_parser("Ljava/lang/Object;")?;
        assert_eq!(format!("{}", d), "Ljava/lang/Object;");
        assert_eq!(rest, "");
        assert_eq!(d, FieldDescriptor::Object(String::from("java/lang/Object")));

        Ok(())
    }

    #[test]
    fn field_descriptor_parser_parses_arrays() -> anyhow::Result<()> {
        let (rest, d) = field_descriptor_parser("[D")?;
        assert_eq!(format!("{}", d), "[D");
        assert_eq!(rest, "");
        assert_eq!(d, FieldDescriptor::Array(Box::new(FieldDescriptor::Double)));

        let (rest, d) = field_descriptor_parser("[[[Ljava/lang/Object;")?;
        assert_eq!(format!("{}", d), "[[[Ljava/lang/Object;");
        assert_eq!(rest, "");
        assert_eq!(
            d,
            FieldDescriptor::Array(Box::new(FieldDescriptor::Array(Box::new(
                FieldDescriptor::Array(Box::new(FieldDescriptor::Object(String::from(
                    "java/lang/Object"
                ))))
            ))))
        );

        Ok(())
    }

    #[test]
    fn return_descriptor_parser_parses_void() -> anyhow::Result<()> {
        let (rest, d) = return_descriptor_parser("V")?;
        assert_eq!(format!("{}", d), "V");
        assert_eq!((rest, d), ("", ReturnDescriptor::Void));
        Ok(())
    }

    #[test]
    fn return_descriptor_parser_parses_fields() -> anyhow::Result<()> {
        let (rest, d) = return_descriptor_parser("B")?;
        assert_eq!(format!("{}", d), "B");
        assert_eq!(rest, "");
        assert_eq!(d, ReturnDescriptor::Field(FieldDescriptor::Byte));

        let (rest, d) = return_descriptor_parser("Ljava/lang/Object;")?;
        assert_eq!(format!("{}", d), "Ljava/lang/Object;");
        assert_eq!(rest, "");
        assert_eq!(
            d,
            ReturnDescriptor::Field(FieldDescriptor::Object(String::from("java/lang/Object")))
        );

        Ok(())
    }

    #[test]
    fn method_descriptor_parser_parses_methods() -> anyhow::Result<()> {
        let (rest, d) = method_descriptor_parser("(IDLjava/lang/Thread;)Ljava/lang/Object;")?;
        assert_eq!(format!("{}", d), "(IDLjava/lang/Thread;)Ljava/lang/Object;");
        assert_eq!(rest, "");
        assert_eq!(
            d,
            MethodDescriptor::new(
                vec![
                    FieldDescriptor::Int,
                    FieldDescriptor::Double,
                    FieldDescriptor::Object(String::from("java/lang/Thread"))
                ],
                ReturnDescriptor::Field(FieldDescriptor::Object(String::from("java/lang/Object")))
            )
        );

        let (rest, d) = method_descriptor_parser("()V")?;
        assert_eq!(format!("{}", d), "()V");
        assert_eq!(rest, "");
        assert_eq!(d, MethodDescriptor::new(vec![], ReturnDescriptor::Void));

        Ok(())
    }
}
