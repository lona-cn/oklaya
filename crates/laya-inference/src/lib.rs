pub mod model;
pub mod runtime;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("model download: {0}")]
    ModelDownload(String),
    #[error("checksum mismatch: {0}")]
    ChecksumMismatch(String),
    #[error("tokenizer load: {0}")]
    TokenizerLoad(String),
    #[error("ORT initialization: {0}")]
    OrtInitialization(String),
    #[error("provider unavailable: {0}")]
    ProviderUnavailable(String),
    #[error("invalid question: {0}")]
    InvalidQuestion(String),
    #[error("invalid model input: {0}")]
    InvalidModelInput(String),
    #[error("inference: {0}")]
    Inference(String),
    #[error("output decode: {0}")]
    OutputDecode(String),
    #[error("calibration: {0}")]
    Calibration(String),
}
pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Question {
    Choice {
        instructions: String,
        criteria: IndexMap<String, String>,
    },
    Score {
        instructions: String,
        criteria: Vec<String>,
    },
    Noul {
        instructions: String,
        #[serde(default)]
        criteria: BTreeMap<String, String>,
    },
}
impl Question {
    pub fn instructions(&self) -> &str {
        match self {
            Self::Choice { instructions, .. }
            | Self::Score { instructions, .. }
            | Self::Noul { instructions, .. } => instructions,
        }
    }
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Choice { .. } => "choice",
            Self::Score { .. } => "score",
            Self::Noul { .. } => "noul",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum DecisionResult {
    Choice {
        choice: String,
        confidence: f64,
        answer_confidence: f64,
        action_probability: f64,
        probabilities: BTreeMap<String, f64>,
    },
    Score {
        score: f64,
        confidence: f64,
        answer_confidence: f64,
        action_probability: f64,
        legend: BTreeMap<String, String>,
        probabilities: BTreeMap<String, f64>,
    },
    Noul {
        value: bool,
        probability: f64,
        confidence: f64,
        answer_confidence: f64,
        action_probability: f64,
    },
}

#[derive(Debug, Deserialize)]
pub struct Request {
    pub state: String,
    pub questions: IndexMap<String, Question>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_request_retains_question_and_option_order() {
        let request: Request = serde_json::from_str(
            r#"{"state":"A","questions":{"route":{"type":"choice","instructions":"Where?","criteria":{"zeta":"first","alpha":"second"}},"urgent":{"type":"noul","instructions":"Urgent?"}}}"#
        ).unwrap();
        assert_eq!(
            request
                .questions
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["route", "urgent"]
        );
        let Question::Choice { criteria, .. } = &request.questions["route"] else {
            panic!("choice lost type")
        };
        assert_eq!(
            criteria.keys().map(String::as_str).collect::<Vec<_>>(),
            ["zeta", "alpha"]
        );
    }
}
