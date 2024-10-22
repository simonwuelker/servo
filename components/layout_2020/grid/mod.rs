/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! <https://drafts.csswg.org/css-grid>

mod construct;
mod geom;
mod layout;

use std::collections::HashSet;

use geom::{GridArea, GridCell};
use serde::Serialize;
use servo_arc::Arc;
use style::properties::ComputedValues;
use style::values::specified::GenericGridTemplateComponent;

use crate::cell::ArcRefCell;
use crate::formatting_contexts::IndependentFormattingContext;

/// <https://drafts.csswg.org/css-grid/#grid-formatting-context>
#[derive(Debug, Serialize)]
pub struct GridFormattingContext {
    #[serde(skip_serializing)]
    style: Arc<ComputedValues>,

    children: Vec<ArcRefCell<GridItemBox>>,

    /// Number of rows in the explicit grid
    explicit_row_count: i32,

    /// Number of columns in the explicit grid
    explicit_column_count: i32,
}

#[derive(Debug, Serialize)]
pub struct GridItemBox {
    independent_formatting_context: IndependentFormattingContext,
}

impl GridItemBox {
    fn style(&self) -> &Arc<ComputedValues> {
        self.independent_formatting_context.style()
    }

    fn is_auto_positioned(&self) -> bool {
        let style = self.style();

        let row_position_is_auto =
            style.clone_grid_row_start().is_auto() && style.clone_grid_row_end().is_auto();
        let column_position_is_auto =
            style.clone_grid_column_start().is_auto() && style.clone_grid_column_end().is_auto();

        row_position_is_auto || column_position_is_auto
    }
}

impl GridFormattingContext {
    fn new(style: Arc<ComputedValues>, children: Vec<ArcRefCell<GridItemBox>>) -> Self {
        // Determine the size of the explicit grid (https://drafts.csswg.org/css-grid/#explicit-grids)
        // FIXME this should take grid-template-areas into account
        let explicit_row_count = style.clone_grid_template_rows().track_list_len() as i32;
        let explicit_column_count = style.clone_grid_template_columns().track_list_len() as i32;

        Self {
            style,
            children,
            explicit_row_count,
            explicit_column_count,
        }
    }
}

/// Tracks occupied cells across an infinitely large grid
#[derive(Debug, Serialize)]
struct OccupationGrid {
    min_row: i32,
    max_row: i32,
    min_column: i32,
    max_column: i32,
    occupied_cells: HashSet<GridCell>,
}

impl OccupationGrid {
    fn new(explicit_row_count: i32, explicit_column_count: i32) -> Self {
        Self {
            min_row: 0,
            max_row: explicit_row_count,
            min_column: 0,
            max_column: explicit_column_count,
            occupied_cells: HashSet::new(),
        }
    }

    fn row_count(&self) -> i32 {
        self.max_row - self.min_row - 1
    }

    fn column_count(&self) -> i32 {
        self.max_column - self.min_column - 1
    }

    /// Place an area occupying one or more cells on the grid
    ///
    /// This automatically expands the grid as necessary
    fn mark_area_as_occupied(&mut self, area: GridArea) {
        self.min_row = self.min_row.min(area.row_start);
        self.max_row = self.max_row.max(area.row_end);
        self.min_column = self.min_column.min(area.column_start);
        self.max_column = self.max_column.max(area.column_end);

        for column in area.column_start..area.column_end {
            for row in area.row_start..area.row_end {
                let position = GridCell { row, column };
                self.occupied_cells.insert(position);
            }
        }
    }

    /// <https://drafts.csswg.org/css-grid/#occupied>
    fn is_cell_occupied(&self, position: GridCell) -> bool {
        self.occupied_cells.contains(&position)
    }
}
