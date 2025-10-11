/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::mem;

use markup5ever::QualName;
use malloc_size_of_derive::MallocSizeOf;
use markup5ever::QualName;

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub enum Expression {
    Binary(Box<Expression>, BinaryOperator, Box<Expression>),
    Negate(Box<Expression>),
    Path(PathExpression),
    /// <https://www.w3.org/TR/1999/REC-xpath-19991116/#section-Location-Steps>
    LocationStep(LocationStepExpression),
    Filter(FilterExpression),
    Literal(Literal),
    Variable(QName),
    ContextItem,
    /// We only support the built-in core functions.
    Function(CoreFunction),
    GetAttributeByName(QualName),
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub enum BinaryOperator {
    Or,
    And,
    Union,
    /// `=`
    Equal,
    /// `!=`
    NotEqual,
    /// `<`
    LessThan,
    /// `>`
    GreaterThan,
    /// `<=`
    LessThanOrEqual,
    /// `>=`
    GreaterThanOrEqual,
    /// `+`
    Add,
    /// `-`
    Subtract,
    /// `*`
    Multiply,
    /// `div`
    Divide,
    /// `mod`
    Modulo,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct PathExpression {
    /// Whether this is an absolute (as opposed to a relative) path expression.
    ///
    /// Absolute paths always start at the starting node, not the context node.
    pub(crate) is_absolute: bool,
    /// Whether this expression starts with `//`. If it does, then an implicit
    /// `descendant-or-self::node()` step will be added.
    pub(crate) has_implicit_descendant_or_self_step: bool,
    pub(crate) steps: Vec<Expression>,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct PredicateListExpression {
    pub(crate) predicates: Vec<Expression>,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct FilterExpression {
    pub(crate) expression: Box<Expression>,
    pub(crate) predicates: PredicateListExpression,
}

/// <https://www.w3.org/TR/1999/REC-xpath-19991116/#section-Location-Steps>
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct LocationStepExpression {
    pub(crate) axis: Axis,
    pub(crate) node_test: NodeTest,
    pub(crate) predicate_list: PredicateListExpression,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub(crate) enum Axis {
    Child,
    Descendant,
    Attribute,
    Self_,
    DescendantOrSelf,
    FollowingSibling,
    Following,
    Namespace,
    Parent,
    Ancestor,
    PrecedingSibling,
    Preceding,
    AncestorOrSelf,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub(crate) enum NodeTest {
    Name(QualName),
    Wildcard,
    Kind(KindTest),
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub struct QName {
    pub(crate) prefix: Option<String>,
    pub(crate) local_part: String,
}

impl std::fmt::Display for QName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.prefix {
            Some(prefix) => write!(f, "{}:{}", prefix, self.local_part),
            None => write!(f, "{}", self.local_part),
        }
    }
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub(crate) enum KindTest {
    PI(Option<String>),
    Comment,
    Text,
    Node,
}

#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub enum Literal {
    Integer(i64),
    Decimal(f64),
    String(String),
}

/// In the DOM we do not support custom functions, so we can enumerate the usable ones
#[derive(Clone, Debug, MallocSizeOf, PartialEq)]
pub enum CoreFunction {
    // Node Set Functions
    /// last()
    Last,
    /// position()
    Position,
    /// count(node-set)
    Count(Box<Expression>),
    /// id(object)
    Id(Box<Expression>),
    /// local-name(node-set?)
    LocalName(Option<Box<Expression>>),
    /// namespace-uri(node-set?)
    NamespaceUri(Option<Box<Expression>>),
    /// name(node-set?)
    Name(Option<Box<Expression>>),

    // String Functions
    /// string(object?)
    String(Option<Box<Expression>>),
    /// concat(string, string, ...)
    Concat(Vec<Expression>),
    /// starts-with(string, string)
    StartsWith(Box<Expression>, Box<Expression>),
    /// contains(string, string)
    Contains(Box<Expression>, Box<Expression>),
    /// substring-before(string, string)
    SubstringBefore(Box<Expression>, Box<Expression>),
    /// substring-after(string, string)
    SubstringAfter(Box<Expression>, Box<Expression>),
    /// substring(string, number, number?)
    Substring(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    /// string-length(string?)
    StringLength(Option<Box<Expression>>),
    /// normalize-space(string?)
    NormalizeSpace(Option<Box<Expression>>),
    /// translate(string, string, string)
    Translate(Box<Expression>, Box<Expression>, Box<Expression>),

    // Number Functions
    /// number(object?)
    Number(Option<Box<Expression>>),
    /// sum(node-set)
    Sum(Box<Expression>),
    /// floor(number)
    Floor(Box<Expression>),
    /// ceiling(number)
    Ceiling(Box<Expression>),
    /// round(number)
    Round(Box<Expression>),

    // Boolean Functions
    /// boolean(object)
    Boolean(Box<Expression>),
    /// not(boolean)
    Not(Box<Expression>),
    /// true()
    True,
    /// false()
    False,
    /// lang(string)
    Lang(Box<Expression>),
}

impl Expression {
    pub(crate) fn optimize(&mut self) {
        match self {
            Self::Binary(left_side, _, right_side) => {
                left_side.optimize();
                right_side.optimize();
            },
            Self::Negate(expression) => {
                expression.optimize();
            },
            Self::Path(path_expression) => {
                path_expression.optimize();
            },
            Self::LocationStep(step_expression) => {
                step_expression.predicate_list.optimize();

                if step_expression.axis == Axis::Attribute &&
                    let NodeTest::Name(name) = &step_expression.node_test
                {
                    // Instead of doing an O(n) search through the list of attributes, do an
                    // O(1) lookup by name.
                    if step_expression.predicate_list.predicates.is_empty() {
                        *self = Self::GetAttributeByName(name.local_part.to_owned());
                    } else {
                        *self = Self::Filter(FilterExpression {
                            expression: Box::new(Self::GetAttributeByName(
                                name.local_part.to_owned(),
                            )),
                            predicates: mem::take(&mut step_expression.predicate_list),
                        })
                    }
                }
            },
            Self::Filter(filter_expression) => {
                filter_expression.expression.optimize();
                filter_expression.predicates.optimize();
            },
            Self::Function(function) => function.optimize(),
            Self::ContextItem |
            Self::GetAttributeByName(_) |
            Self::Literal(_) |
            Self::Variable(_) => {},
        }
    }
}

impl PathExpression {
    fn optimize(&mut self) {
        for step in &mut self.steps {
            step.optimize();
        }
    }
}

impl PredicateListExpression {
    fn optimize(&mut self) {
        for expression in &mut self.predicates {
            expression.optimize();
        }
    }
}

impl CoreFunction {
    fn optimize(&mut self) {
        match self {
            Self::Last | Self::Position | Self::True | Self::False => {},
            Self::Count(expression) |
            Self::Id(expression) |
            Self::Sum(expression) |
            Self::Floor(expression) |
            Self::Ceiling(expression) |
            Self::Round(expression) |
            Self::Boolean(expression) |
            Self::Not(expression) |
            Self::Lang(expression) => expression.optimize(),
            Self::LocalName(optional_expression) |
            Self::NamespaceUri(optional_expression) |
            Self::Name(optional_expression) |
            Self::String(optional_expression) |
            Self::StringLength(optional_expression) |
            Self::NormalizeSpace(optional_expression) |
            Self::Number(optional_expression) => {
                if let Some(expression) = optional_expression {
                    expression.optimize();
                }
            },
            Self::Concat(expressions) => {
                for expression in expressions {
                    expression.optimize();
                }
            },
            Self::StartsWith(first_argument, second_argument) |
            Self::Contains(first_argument, second_argument) |
            Self::SubstringBefore(first_argument, second_argument) |
            Self::SubstringAfter(first_argument, second_argument) => {
                first_argument.optimize();
                second_argument.optimize();
            },
            Self::Substring(source_expression, position_expression, length_expression) => {
                source_expression.optimize();
                position_expression.optimize();
                if let Some(length_expression) = length_expression {
                    length_expression.optimize();
                }
            },
            Self::Translate(source_expression, to_replace_expression, replace_with_expression) => {
                source_expression.optimize();
                to_replace_expression.optimize();
                replace_with_expression.optimize();
            },
        }
    }
}
