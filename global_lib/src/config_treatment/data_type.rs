use crate::Step;

use super::result_fields::ResultDuration;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Error as FmtErr, Formatter};
pub use std::mem::ManuallyDrop;
use std::str::FromStr;

#[derive(Eq, Copy, Hash, PartialEq, Clone, Debug, Deserialize, Serialize)]
pub enum DataType {
    Average,
    Median,
    Details,
}

pub union ProcessResult {
    val: ResultDuration,
    details: ManuallyDrop<String>,
}

impl ProcessResult {
    pub fn details(details: ManuallyDrop<String>) -> ProcessResult {
        ProcessResult { details }
    }

    pub fn val(val: ResultDuration) -> ProcessResult {
        ProcessResult { val }
    }

    pub fn get_details(&self) -> &String {
        unsafe { &self.details }
    }

    pub fn get_val(&self) -> ResultDuration {
        unsafe { self.val }
    }
}

impl FromStr for DataType {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<DataType, Self::Err> {
        Ok(match s {
            "average" => DataType::Average,
            "median" => DataType::Median,
            "details" => DataType::Details,
            _ => return Err("invalid data result"),
        })
    }
}

impl DataType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DataType::Average => "average",
            DataType::Median => "median",
            DataType::Details => "details",
        }
    }

    pub fn process(&self, step: Step, datas: &Vec<ResultDuration>) -> ProcessResult {
        match self {
            Self::Average => ProcessResult::val(
                datas.iter().sum::<ResultDuration>() / datas.len() as ResultDuration,
            ),
            Self::Median => ProcessResult::val(datas[datas.len() / 2]),
            Self::Details => ProcessResult::details(ManuallyDrop::new(format!(
                "{step:?}: {datas:?}, average: {}, median: {}",
                DataType::Average.process(step, datas).get_val(),
                DataType::Average.process(step, datas).get_val()
            ))),
        }
    }
}

impl Display for DataType {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), FmtErr> {
        write!(f, "{}", self.as_str())
    }
}
