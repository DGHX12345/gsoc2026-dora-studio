//! Multi-project dataflow discovery (M18). Built in Task 1.3.
use crate::dataflows::{DataflowError, DataflowFile};

pub(crate) fn find_dataflow_file(id: &str) -> Result<DataflowFile, DataflowError> {
    Err(DataflowError::NotFound(format!(
        "Dataflow '{id}' was not found."
    )))
}
