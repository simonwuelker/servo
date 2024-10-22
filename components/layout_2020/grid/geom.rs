/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GridDimension {
    Row,
    Column,
}

/// <https://drafts.csswg.org/css-grid/#grid-cell>
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
pub struct GridCell {
    pub row: i32,
    pub column: i32,
}

/// <https://drafts.csswg.org/css-grid/#grid-area>
#[derive(Clone, Copy, Debug, Serialize)]
pub struct GridArea {
    pub row_start: i32,
    pub row_end: i32,
    pub column_start: i32,
    pub column_end: i32,
}
