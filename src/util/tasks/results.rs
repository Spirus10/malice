use std::io::{Error, ErrorKind, Result};

use super::types::{TaskResultData, TaskResultPayload, TaskStatus};

pub fn decode_result(payload: TaskResultPayload) -> Result<(uuid::Uuid, TaskStatus, TaskResultData)> {
    let status = if payload.status.eq_ignore_ascii_case("success") {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };

    if payload.result_encoding.is_empty() {
        return Err(Error::new(ErrorKind::InvalidInput, "Missing result encoding"));
    }

    Ok((
        payload.task_id,
        status,
        TaskResultData::Text {
            encoding: payload.result_encoding,
            data: payload.result_data,
        },
    ))
}
