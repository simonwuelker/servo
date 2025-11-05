/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use std::mem;

use crate::ast::{
    Axis, CoreFunction, Expression, FilterExpression, LocationStepExpression, PathExpression, PredicateListExpression
};

/// Enumerates factors that can influence the value that an expression evaluates to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InfluencingFactor {
    /// The value that the expression evaluates to depends on the set of context nodes.
    ///
    /// For example, this is true for `last()`.
    ContextSize,
    /// The value that the expression evaluates to depends on position of the node in the set
    /// of context nodes.
    ///
    /// For example, this is true for `position() = 3`.
    ContextPosition,
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
                step_expression.optimize();
            },
            Self::Filter(filter_expression) => filter_expression.optimize(),
            Self::Function(function) => function.optimize(),
            Self::ContextItem | Self::Literal(_) | Self::Variable(_) => {},
        }
    }

    fn is_influenced_by(&self, factor: InfluencingFactor) -> bool {
        match self {
            Self::Binary(left_side, _, right_side) => {
                left_side.is_influenced_by(factor) | right_side.is_influenced_by(factor)
            },
            Self::Negate(expression) => expression.is_influenced_by(factor),
            Self::Path(path_expression) => path_expression.is_influenced_by(factor),
            Self::LocationStep(step_expression) => step_expression.is_influenced_by(factor),
            Self::Filter(filter_expression) => filter_expression.is_influenced_by(factor),
            Self::Function(function) => function.is_influenced_by(factor),
            Self::ContextItem | Self::Literal(_) | Self::Variable(_) => false,
        }
    }
}

impl PathExpression {
    fn optimize(&mut self) {
        for step in &mut self.steps {
            step.optimize();
        }
        // return;

        // Optimize an implicit descendant-or-self-step followed by a location step with
        // axis "child" to a location step with axis "descendant".
        if self.has_implicit_descendant_or_self_step {
            if let Some(Expression::LocationStep(entry)) = self.steps.first_mut() {
                if entry.axis == Axis::Child {
                    entry.axis = Axis::Descendant;
                    self.has_implicit_descendant_or_self_step = false;
                } else if matches!(entry.axis, Axis::Descendant | Axis::DescendantOrSelf) {
                    self.has_implicit_descendant_or_self_step = false;
                }
            }
        }
    }

    fn is_influenced_by(&self, _: InfluencingFactor) -> bool {
        // Path expressions create new context node sets, so they're
        false
    }
}

impl FilterExpression {
    fn optimize(&mut self) {
        self.expression.optimize();
        self.predicates.optimize();
    }

    fn is_influenced_by(&self, factor: InfluencingFactor) -> bool {
        self.expression.is_influenced_by(factor) | self.predicates.is_influenced_by(factor)
    }
}

impl LocationStepExpression {
    fn optimize(&mut self) {
        self.predicate_list.optimize();
    }

    fn is_influenced_by(&self, factor: InfluencingFactor) -> bool {
        self.predicate_list.is_influenced_by(factor)
    }
}

impl PredicateListExpression {
    fn optimize(&mut self) {
        for expression in &mut self.predicates {
            expression.optimize();
        }
    }

    fn is_influenced_by(&self, factor: InfluencingFactor) -> bool {
        self.predicates
            .iter()
            .any(|predicate| predicate.is_influenced_by(factor))
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

    fn is_influenced_by(&self, factor: InfluencingFactor) -> bool {
        let is_inherently_dependent = match self {
            Self::Last => factor == InfluencingFactor::ContextSize,
            Self::Position => factor == InfluencingFactor::ContextPosition,
            _ => false,
        };

        let arguments_are_influenced_by_factor = || match self {
            Self::Last | Self::Position | Self::True | Self::False => false,
            Self::Count(expression) |
            Self::Id(expression) |
            Self::Sum(expression) |
            Self::Floor(expression) |
            Self::Ceiling(expression) |
            Self::Round(expression) |
            Self::Boolean(expression) |
            Self::Not(expression) |
            Self::Lang(expression) => expression.is_influenced_by(factor),
            Self::LocalName(optional_expression) |
            Self::NamespaceUri(optional_expression) |
            Self::Name(optional_expression) |
            Self::String(optional_expression) |
            Self::StringLength(optional_expression) |
            Self::NormalizeSpace(optional_expression) |
            Self::Number(optional_expression) => optional_expression
                .as_ref()
                .is_some_and(|expression| expression.is_influenced_by(factor)),
            Self::Concat(expressions) => expressions
                .iter()
                .any(|expression| expression.is_influenced_by(factor)),
            Self::StartsWith(first_argument, second_argument) |
            Self::Contains(first_argument, second_argument) |
            Self::SubstringBefore(first_argument, second_argument) |
            Self::SubstringAfter(first_argument, second_argument) => {
                first_argument.is_influenced_by(factor) || second_argument.is_influenced_by(factor)
            },
            Self::Substring(source_expression, position_expression, length_expression) => {
                source_expression.is_influenced_by(factor) ||
                    position_expression.is_influenced_by(factor) ||
                    length_expression
                        .as_ref()
                        .is_some_and(|expression| expression.is_influenced_by(factor))
            },
            Self::Translate(source_expression, to_replace_expression, replace_with_expression) => {
                source_expression.is_influenced_by(factor) ||
                    to_replace_expression.is_influenced_by(factor) ||
                    replace_with_expression.is_influenced_by(factor)
            },
        };

        is_inherently_dependent || arguments_are_influenced_by_factor()
    }
}
