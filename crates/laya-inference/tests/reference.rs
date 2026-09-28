use indexmap::IndexMap;
use laya_inference::{
    DecisionResult, Question,
    model::ModelKind,
    runtime::{Device, Laya},
};
use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
struct Case {
    id: String,
    state: String,
    questions: IndexMap<String, Question>,
    answers: BTreeMap<String, serde_json::Value>,
}

fn close(actual: f64, expected: f64, context: &str) {
    assert!(
        (actual - expected).abs() <= 0.002,
        "{context}: {actual} != {expected}"
    );
}

#[test]
fn official_onnx_agent_parity() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("LAYA_RUN_MODEL_TESTS").is_none() {
        return Ok(());
    }
    let mut builder = Laya::builder()
        .model(ModelKind::Multilingual)
        .device(Device::Cpu);
    if let Some(path) = std::env::var_os("LAYA_MODEL_DIR") {
        builder = builder.model_path(PathBuf::from(path));
    }
    let mut engine = builder.build()?;
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/reference.json"))?;
    assert!(cases.len() >= 20);
    for case in cases {
        let results = engine.predict(&case.state, &case.questions)?;
        for (name, expected) in case.answers {
            let result = &results[&name];
            let ctx = format!("{} / {name}", case.id);
            close(
                result_answer_confidence(result),
                expected["answer_confidence"]
                    .as_f64()
                    .ok_or("missing confidence")?,
                &ctx,
            );
            let action = match result {
                DecisionResult::Choice {
                    action_probability, ..
                }
                | DecisionResult::Score {
                    action_probability, ..
                }
                | DecisionResult::Noul {
                    action_probability, ..
                } => *action_probability,
            };
            close(
                action,
                expected["action"]["act_probability"]
                    .as_f64()
                    .ok_or("missing action")?,
                &ctx,
            );
            match result {
                DecisionResult::Choice {
                    choice,
                    probabilities,
                    confidence,
                    ..
                } => {
                    assert_eq!(
                        choice,
                        expected["choice"].as_str().ok_or("missing choice")?,
                        "{ctx}"
                    );
                    close(
                        *confidence,
                        expected["confidence"]
                            .as_f64()
                            .ok_or("missing confidence")?,
                        &ctx,
                    );
                    for (key, p) in probabilities {
                        close(
                            *p,
                            expected["probabilities"][key]
                                .as_f64()
                                .ok_or("missing probability")?,
                            &ctx,
                        );
                    }
                }
                DecisionResult::Score {
                    score,
                    probabilities,
                    confidence,
                    legend,
                    ..
                } => {
                    close(
                        *score,
                        expected["score"].as_f64().ok_or("missing score")?,
                        &ctx,
                    );
                    close(
                        *confidence,
                        expected["confidence"]
                            .as_f64()
                            .ok_or("missing confidence")?,
                        &ctx,
                    );
                    for (key, value) in legend {
                        assert_eq!(
                            value,
                            expected["legend"][key].as_str().ok_or("missing legend")?,
                            "{ctx}"
                        );
                    }
                    for (key, p) in probabilities {
                        close(
                            *p,
                            expected["probabilities"][key]
                                .as_f64()
                                .ok_or("missing probability")?,
                            &ctx,
                        );
                    }
                }
                DecisionResult::Noul {
                    probability,
                    confidence,
                    value,
                    ..
                } => {
                    let truth = expected["noul"].as_f64().ok_or("missing noul")?;
                    close(*probability, truth, &ctx);
                    close(
                        *confidence,
                        expected["confidence"]
                            .as_f64()
                            .ok_or("missing confidence")?,
                        &ctx,
                    );
                    assert_eq!(*value, truth >= 0.5, "{ctx}");
                }
            }
        }
    }
    Ok(())
}
fn result_answer_confidence(result: &DecisionResult) -> f64 {
    match result {
        DecisionResult::Choice {
            answer_confidence, ..
        }
        | DecisionResult::Score {
            answer_confidence, ..
        }
        | DecisionResult::Noul {
            answer_confidence, ..
        } => *answer_confidence,
    }
}

#[test]
fn multilingual_token_ids_match_official_fast_tokenizer()
-> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    if std::env::var_os("LAYA_RUN_MODEL_TESTS").is_none() {
        return Ok(());
    }
    let dir = std::env::var_os("LAYA_MODEL_DIR")
        .map(PathBuf::from)
        .unwrap_or(laya_inference::model::model_dir(ModelKind::Multilingual)?);
    let tokenizer = tokenizers::Tokenizer::from_file(dir.join("tokenizer/tokenizer.json"))?;
    let cases: [(&str, &[u32]); 5] = [
        (
            "choice question: Which department should handle this?",
            &[6241, 2872, 235292, 12236, 9888, 1412, 6589, 736, 235336],
        ),
        (
            " billing: payments and refunds",
            &[54972, 235292, 15598, 578, 85869],
        ),
        (
            " technical: bugs and crashes",
            &[9838, 235292, 30608, 578, 52688],
        ),
        (" other: everything else", &[1156, 235292, 4553, 1354]),
        (
            "The application crashes whenever I open settings.",
            &[714, 4724, 52688, 18264, 590, 2174, 8791, 235265],
        ),
    ];
    for (text, expected) in cases {
        let actual = tokenizer.encode(text, false)?;
        assert_eq!(actual.get_ids(), expected, "tokenizer mismatch on {text:?}");
        assert_eq!(actual.get_attention_mask(), vec![1; expected.len()]);
    }
    Ok(())
}
