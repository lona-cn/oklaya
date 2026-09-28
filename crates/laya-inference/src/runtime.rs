//! Native tokenizer, batching, ONNX inference and calibrated typed decisions.
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use ort::{
    ep::{CUDA, ExecutionProvider},
    session::{Session, builder::GraphOptimizationLevel},
    value::{Tensor, TensorElementType},
};
use serde::Deserialize;
use tokenizers::Tokenizer;

use crate::{
    DecisionResult, Error, Question, Result,
    model::{self, ModelKind},
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Device {
    #[default]
    Auto,
    Cpu,
    Cuda,
}

#[derive(Debug, Deserialize)]
struct Config {
    #[serde(default = "default_max_len")]
    max_len: usize,
    #[serde(default = "default_head_len")]
    head_max_len: usize,
    #[serde(default = "default_temperatures")]
    temperature: [f64; 3],
    #[serde(default)]
    temperature_by_options: BTreeMap<String, f64>,
}
fn default_max_len() -> usize {
    1024
}
fn default_head_len() -> usize {
    192
}
fn default_temperatures() -> [f64; 3] {
    [1.0; 3]
}

#[derive(Debug, Clone, Copy)]
struct Special {
    cls: i64,
    sep: i64,
    mask: i64,
    pad: i64,
}

pub struct LayaBuilder {
    model: ModelKind,
    model_path: Option<PathBuf>,
    device: Device,
    max_len: Option<usize>,
}
pub struct Laya {
    session: Session,
    tokenizer: Tokenizer,
    special: Special,
    mask_text: String,
    config: Config,
    max_len: usize,
    model_path: PathBuf,
    provider: &'static str,
}

impl Laya {
    pub fn builder() -> LayaBuilder {
        LayaBuilder {
            model: ModelKind::Multilingual,
            model_path: None,
            device: Device::Auto,
            max_len: None,
        }
    }
    pub fn provider(&self) -> &str {
        self.provider
    }
    pub fn model_path(&self) -> &Path {
        &self.model_path
    }

    /// Runs one ONNX forward pass across every question, preserving input order.
    pub fn predict(
        &mut self,
        state: &str,
        questions: &IndexMap<String, Question>,
    ) -> Result<IndexMap<String, DecisionResult>> {
        if questions.is_empty() {
            return Ok(IndexMap::new());
        }
        let state_ids = self.encode(state)?;
        let mut rows = Vec::with_capacity(questions.len());
        for (name, question) in questions {
            rows.push(self.build_row(name, question, &state_ids)?);
        }
        let temperatures: Vec<f64> = questions
            .iter()
            .zip(&rows)
            .map(|((_, q), row)| self.temperature(q.kind(), row.markers.len()))
            .collect();
        let n = rows.len();
        // This exported graph needs at least eight sequence positions and two marker slots.
        let width = rows.iter().map(|row| row.ids.len()).max().unwrap().max(8);
        let options = rows
            .iter()
            .map(|row| row.markers.len())
            .max()
            .unwrap()
            .max(2);
        let mut ids = vec![self.special.pad; n * width];
        let mut attention = vec![0_i64; n * width];
        let mut markers = vec![0_i64; n * options];
        let mut marker_mask = vec![false; n * options];
        let mut qtype = Vec::with_capacity(n);
        for (i, row) in rows.iter().enumerate() {
            let offset = i * width;
            ids[offset..offset + row.ids.len()].copy_from_slice(&row.ids);
            attention[offset..offset + row.ids.len()].fill(1);
            markers[i * options..i * options + row.markers.len()].copy_from_slice(&row.markers);
            marker_mask[i * options..i * options + row.markers.len()].fill(true);
            qtype.push(row.qtype);
        }
        let shape = [n, width];
        let marker_shape = [n, options];
        let run_options = ort::session::RunOptions::new()
            .map_err(|e| Error::Inference(e.to_string()))?
            .with_outputs(
                ort::session::OutputSelector::no_default()
                    .with("logits")
                    .with("act_logits"),
            );
        let output = self.session.run_with_options(ort::inputs! {
            "input_ids" => Tensor::from_array((shape, ids)).map_err(|e| Error::InvalidModelInput(e.to_string()))?,
            "attention_mask" => Tensor::from_array((shape, attention)).map_err(|e| Error::InvalidModelInput(e.to_string()))?,
            "marker_pos" => Tensor::from_array((marker_shape, markers)).map_err(|e| Error::InvalidModelInput(e.to_string()))?,
            "marker_mask" => Tensor::from_array((marker_shape, marker_mask)).map_err(|e| Error::InvalidModelInput(e.to_string()))?,
            "qtype" => Tensor::from_array(([n], qtype)).map_err(|e| Error::InvalidModelInput(e.to_string()))?,
        }, &run_options).map_err(|e| Error::Inference(e.to_string()))?;
        let logits = output
            .get("logits")
            .ok_or_else(|| Error::OutputDecode("missing logits".into()))?;
        let (logit_shape, logits) = logits
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::OutputDecode(e.to_string()))?;
        if logit_shape.len() != 2 || logit_shape[0] != n as i64 || logit_shape[1] < options as i64 {
            return Err(Error::OutputDecode(format!(
                "logits shape {logit_shape:?}, expected [{n}, at least {options}]"
            )));
        }
        let act = output
            .get("act_logits")
            .ok_or_else(|| Error::OutputDecode("missing act_logits".into()))?;
        let (act_shape, act_logits) = act
            .try_extract_tensor::<f32>()
            .map_err(|e| Error::OutputDecode(e.to_string()))?;
        if act_shape.len() != 2 || act_shape[0] != n as i64 || act_shape[1] != 2 {
            return Err(Error::OutputDecode(format!(
                "act_logits shape {act_shape:?}, expected [{n}, 2]"
            )));
        }
        let stride = logit_shape[1] as usize;
        let mut results = IndexMap::with_capacity(n);
        for ((name, question), (i, row)) in questions.iter().zip(rows.iter().enumerate()) {
            let k = row.markers.len();
            let temperature = temperatures[i];
            let probs = softmax(&logits[i * stride..i * stride + k], temperature)?;
            let answer_confidence = round4(probs.iter().copied().fold(0.0, f64::max));
            let confidence = round4(entropy_confidence(&probs));
            let action_delta = f64::from(act_logits[i * 2 + 1]) - f64::from(act_logits[i * 2]);
            if !action_delta.is_finite() {
                return Err(Error::OutputDecode("non-finite action logits".into()));
            }
            let action_probability = round4(1.0 / (1.0 + action_delta.exp()));
            let result = match question {
                Question::Choice { criteria, .. } => {
                    let winner = probs
                        .iter()
                        .enumerate()
                        .fold((0, f64::NEG_INFINITY), |best, (index, &probability)| {
                            if probability > best.1 {
                                (index, probability)
                            } else {
                                best
                            }
                        })
                        .0;
                    DecisionResult::Choice {
                        choice: criteria.get_index(winner).unwrap().0.clone(),
                        confidence,
                        answer_confidence,
                        action_probability,
                        probabilities: criteria
                            .keys()
                            .cloned()
                            .zip(probs.iter().copied().map(round4))
                            .collect(),
                    }
                }
                Question::Score { criteria, .. } => DecisionResult::Score {
                    score: round4(
                        probs
                            .iter()
                            .enumerate()
                            .map(|(index, p)| index as f64 * p)
                            .sum(),
                    ),
                    confidence,
                    answer_confidence,
                    action_probability,
                    legend: criteria
                        .iter()
                        .enumerate()
                        .map(|(index, value)| (index.to_string(), value.clone()))
                        .collect(),
                    probabilities: probs
                        .iter()
                        .enumerate()
                        .map(|(index, p)| (index.to_string(), round4(*p)))
                        .collect(),
                },
                Question::Noul { .. } => DecisionResult::Noul {
                    value: probs[1] >= probs[0],
                    probability: round4(probs[1]),
                    confidence: round4(probs[0].max(probs[1])),
                    answer_confidence,
                    action_probability,
                },
            };
            results.insert(name.clone(), result);
        }
        Ok(results)
    }

    pub fn predict_many(
        &mut self,
        state: &str,
        questions: &IndexMap<String, Question>,
    ) -> Result<IndexMap<String, DecisionResult>> {
        self.predict(state, questions)
    }

    fn encode(&self, text: &str) -> Result<Vec<i64>> {
        // Literal mask symbols in user content must never become answer markers.
        let sanitized = if text.contains(&self.mask_text) {
            Cow::Owned(text.replace(&self.mask_text, " "))
        } else {
            Cow::Borrowed(text)
        };
        let encoding = self
            .tokenizer
            .encode(sanitized.as_ref(), false)
            .map_err(|e| Error::TokenizerLoad(e.to_string()))?;
        Ok(encoding.get_ids().iter().map(|&id| i64::from(id)).collect())
    }

    fn build_row(&self, name: &str, question: &Question, state_ids: &[i64]) -> Result<Row> {
        let (qtype, options): (i64, Vec<String>) = match question {
            Question::Choice { criteria, .. } => (
                0,
                criteria
                    .iter()
                    .map(|(label, description)| {
                        if description.is_empty() {
                            label.clone()
                        } else {
                            format!("{label}: {description}")
                        }
                    })
                    .collect(),
            ),
            Question::Score { criteria, .. } => (
                1,
                criteria
                    .iter()
                    .enumerate()
                    .map(|(i, criterion)| format!("level {i}: {criterion}"))
                    .collect(),
            ),
            Question::Noul { criteria, .. } => {
                if criteria.keys().any(|key| key != "true" && key != "false") {
                    return Err(Error::InvalidQuestion(format!(
                        "{name}: noul criteria accepts only false/true"
                    )));
                }
                let false_desc = criteria
                    .get("false")
                    .filter(|s| !s.is_empty())
                    .map(String::as_str)
                    .unwrap_or("no, the statement does not hold");
                let true_desc = criteria
                    .get("true")
                    .filter(|s| !s.is_empty())
                    .map(String::as_str)
                    .unwrap_or("yes, the statement holds");
                (
                    2,
                    vec![format!("false: {false_desc}"), format!("true: {true_desc}")],
                )
            }
        };
        if options.is_empty() {
            return Err(Error::InvalidQuestion(format!(
                "{name}: at least one option is required"
            )));
        }
        if options.len() > i64::MAX as usize {
            return Err(Error::InvalidQuestion(format!("{name}: too many options")));
        }
        let mut option_ids = Vec::with_capacity(options.len());
        for option in &options {
            let mut tokens = self.encode(&format!(" {option}"))?;
            if tokens.len() > 48 {
                eprintln!("laya: option in question {name:?} truncated to 48 tokens");
                tokens.truncate(48);
            }
            let mut span = Vec::with_capacity(tokens.len() + 1);
            span.push(self.special.mask);
            span.extend(tokens);
            option_ids.push(span);
        }
        let mut options_len: usize = option_ids.iter().map(Vec::len).sum();
        if self.config.head_max_len.saturating_sub(options_len) < 16 {
            let per = (self.config.head_max_len.saturating_sub(16) / option_ids.len()).max(4);
            for span in &mut option_ids {
                span.truncate(per);
            }
            options_len = option_ids.iter().map(Vec::len).sum();
            eprintln!("laya: question {name:?} option spans capped to {per} tokens by head budget");
        }
        let mut head = self.encode(&format!(
            "{} question: {}",
            question.kind(),
            question.instructions()
        ))?;
        let head_budget = self.config.head_max_len.saturating_sub(options_len).max(8);
        if head.len() > head_budget {
            eprintln!(
                "laya: question {name:?} instructions truncated from {} to {head_budget} tokens",
                head.len()
            );
            head.truncate(head_budget);
        }
        assemble_row(
            self.special,
            head,
            option_ids,
            state_ids,
            self.max_len,
            name,
            qtype,
        )
    }

    fn temperature(&self, kind: &str, options: usize) -> f64 {
        temperature_for(&self.config, kind, options)
    }
}

fn temperature_for(config: &Config, kind: &str, options: usize) -> f64 {
    let bucket = if options <= 2 {
        "2"
    } else if options <= 5 {
        "3-5"
    } else if options <= 10 {
        "6-10"
    } else {
        "11+"
    };
    let index = match kind {
        "choice" => 0,
        "score" => 1,
        _ => 2,
    };
    let raw = config
        .temperature_by_options
        .get(&format!("{kind}:{bucket}"))
        .copied()
        .unwrap_or(config.temperature[index]);
    clamp_temperature(raw)
}

struct Row {
    ids: Vec<i64>,
    markers: Vec<i64>,
    qtype: i64,
}

fn assemble_row(
    special: Special,
    head: Vec<i64>,
    option_ids: Vec<Vec<i64>>,
    state_ids: &[i64],
    max_len: usize,
    name: &str,
    qtype: i64,
) -> Result<Row> {
    let options_len: usize = option_ids.iter().map(Vec::len).sum();
    // CLS + head + SEP + option spans + SEP + final state SEP.
    let min_len = head.len() + options_len + 3;
    if min_len > max_len {
        return Err(Error::InvalidQuestion(format!(
            "{name}: question needs {min_len} tokens, max_len={max_len}"
        )));
    }
    let mut ids = Vec::with_capacity(max_len.min(min_len + state_ids.len()));
    ids.push(special.cls);
    ids.extend(head);
    ids.push(special.sep);
    let mut markers = Vec::with_capacity(option_ids.len());
    for span in option_ids {
        markers.push(ids.len() as i64);
        ids.extend(span);
    }
    ids.push(special.sep);
    let room = max_len - ids.len() - 1;
    if state_ids.len() > room {
        eprintln!(
            "laya: state truncated from {} to {room} tokens for question {name:?}",
            state_ids.len()
        );
    }
    ids.extend_from_slice(&state_ids[..state_ids.len().min(room)]);
    ids.push(special.sep);
    Ok(Row {
        ids,
        markers,
        qtype,
    })
}

impl LayaBuilder {
    pub fn model(mut self, model: ModelKind) -> Self {
        self.model = model;
        self
    }
    /// Use an existing model directory containing model.onnx, config and tokenizer files.
    /// Unlike `model`, this explicit local path is not downloaded or checksum-verified.
    pub fn model_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.model_path = Some(path.into());
        self
    }
    pub fn device(mut self, device: Device) -> Self {
        self.device = device;
        self
    }
    pub fn max_len(mut self, max_len: usize) -> Self {
        self.max_len = Some(max_len);
        self
    }
    pub fn build(self) -> Result<Laya> {
        let model_path = if let Some(dir) = self.model_path {
            for relative in [
                "model.onnx",
                "rl_agent_config.json",
                "tokenizer/tokenizer.json",
                "tokenizer/tokenizer_config.json",
            ] {
                let file = dir.join(relative);
                if !file.is_file() {
                    return Err(Error::InvalidModelInput(format!(
                        "local model is missing {}",
                        file.display()
                    )));
                }
            }
            dir.join("model.onnx")
        } else {
            model::download(self.model)?
        };
        let dir = model_path
            .parent()
            .ok_or_else(|| Error::InvalidModelInput("model path has no directory".into()))?;
        let config: Config = serde_json::from_slice(
            &fs::read(dir.join("rl_agent_config.json"))
                .map_err(|e| Error::Calibration(e.to_string()))?,
        )
        .map_err(|e| Error::Calibration(e.to_string()))?;
        for (name, raw) in config
            .temperature
            .iter()
            .enumerate()
            .map(|(i, v)| (format!("temperature[{i}]"), *v))
            .chain(
                config
                    .temperature_by_options
                    .iter()
                    .map(|(key, v)| (key.clone(), *v)),
            )
        {
            let applied = clamp_temperature(raw);
            if raw != applied {
                eprintln!(
                    "laya: calibration {name}={raw} is outside [0.5, 5]; applying {applied} (confidence may be uncalibrated)"
                );
            }
        }
        let max_len = self.max_len.unwrap_or(config.max_len);
        if max_len < 8 || config.head_max_len < 8 {
            return Err(Error::InvalidModelInput(
                "max_len and head_max_len must be at least 8".into(),
            ));
        }
        let tokenizer = Tokenizer::from_file(dir.join("tokenizer/tokenizer.json"))
            .map_err(|e| Error::TokenizerLoad(e.to_string()))?;
        let token_cfg: serde_json::Value = serde_json::from_slice(
            &fs::read(dir.join("tokenizer/tokenizer_config.json"))
                .map_err(|e| Error::TokenizerLoad(e.to_string()))?,
        )
        .map_err(|e| Error::TokenizerLoad(e.to_string()))?;
        let token = |field: &str| -> Result<i64> {
            let literal = token_cfg
                .get(field)
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::TokenizerLoad(format!("missing {field} token")))?;
            tokenizer
                .token_to_id(literal)
                .map(i64::from)
                .ok_or_else(|| {
                    Error::TokenizerLoad(format!("token {literal:?} absent from tokenizer"))
                })
        };
        let special = Special {
            cls: token("cls_token")?,
            sep: token("sep_token")?,
            mask: token("mask_token")?,
            pad: token("pad_token")?,
        };
        let mask_text = token_cfg["mask_token"].as_str().unwrap().to_string();
        let cuda = CUDA::default();
        let available = if self.device == Device::Cpu {
            false
        } else {
            match cuda.is_available() {
                Ok(value) => value,
                Err(err) if self.device == Device::Auto => {
                    eprintln!("laya: CUDA availability check failed ({err}); using CPU");
                    false
                }
                Err(err) => return Err(Error::ProviderUnavailable(err.to_string())),
            }
        };
        if self.device == Device::Cuda && !available {
            return Err(Error::ProviderUnavailable(
                "CUDA execution provider is not available".into(),
            ));
        }
        if self.device == Device::Auto && !available {
            eprintln!("laya: CUDA execution provider unavailable; using CPU");
        }
        let cpu_session = || -> Result<Session> {
            let builder =
                Session::builder().map_err(|e| Error::OrtInitialization(e.to_string()))?;
            let mut builder = builder
                .with_optimization_level(GraphOptimizationLevel::Level3)
                .map_err(|e| Error::OrtInitialization(e.to_string()))?;
            builder
                .commit_from_file(&model_path)
                .map_err(|e| Error::OrtInitialization(e.to_string()))
        };
        let (session, provider) = if available {
            // error_on_failure forbids ORT's implicit fallback when CUDA registration fails.
            let attempted = (|| -> Result<Session> {
                let builder =
                    Session::builder().map_err(|e| Error::OrtInitialization(e.to_string()))?;
                let builder = builder
                    .with_optimization_level(GraphOptimizationLevel::Level3)
                    .map_err(|e| Error::OrtInitialization(e.to_string()))?;
                let mut builder = builder
                    .with_execution_providers([cuda.build().error_on_failure()])
                    .map_err(|e| Error::OrtInitialization(e.to_string()))?;
                builder
                    .commit_from_file(&model_path)
                    .map_err(|e| Error::OrtInitialization(e.to_string()))
            })();
            match attempted.and_then(|mut session| {
                inspect_graph(&session)?;
                probe_cuda(&mut session, special)?;
                Ok(session)
            }) {
                Ok(session) => (session, "cuda"),
                Err(err) if self.device == Device::Auto => {
                    eprintln!("laya: CUDA initialization failed ({err}); falling back to CPU");
                    (cpu_session()?, "cpu")
                }
                Err(err) => {
                    return Err(Error::ProviderUnavailable(format!(
                        "CUDA initialization failed: {err}"
                    )));
                }
            }
        } else {
            (cpu_session()?, "cpu")
        };
        inspect_graph(&session)?;
        Ok(Laya {
            session,
            tokenizer,
            special,
            mask_text,
            config,
            max_len,
            model_path,
            provider,
        })
    }
}

/// Probe real kernels, not merely provider registration: some ORT builds lack sm_120 images.
fn probe_cuda(session: &mut Session, special: Special) -> Result<()> {
    let output = session.run(ort::inputs! {
        "input_ids" => Tensor::from_array(([1, 8], vec![special.cls, special.sep, special.mask, special.mask, special.sep, special.sep, special.pad, special.pad])).map_err(|e| Error::Inference(e.to_string()))?,
        "attention_mask" => Tensor::from_array(([1, 8], vec![1_i64, 1, 1, 1, 1, 1, 0, 0])).map_err(|e| Error::Inference(e.to_string()))?,
        "marker_pos" => Tensor::from_array(([1, 2], vec![2_i64, 3])).map_err(|e| Error::Inference(e.to_string()))?,
        "marker_mask" => Tensor::from_array(([1, 2], vec![true, true])).map_err(|e| Error::Inference(e.to_string()))?,
        "qtype" => Tensor::from_array(([1], vec![0_i64])).map_err(|e| Error::Inference(e.to_string()))?,
    }).map_err(|e| Error::ProviderUnavailable(format!("CUDA kernel preflight failed: {e}")))?;
    if output.get("logits").is_none() || output.get("act_logits").is_none() {
        return Err(Error::ProviderUnavailable(
            "CUDA preflight did not produce decision outputs".into(),
        ));
    }
    Ok(())
}

fn inspect_graph(session: &Session) -> Result<()> {
    let required = [
        ("input_ids", TensorElementType::Int64, 2),
        ("attention_mask", TensorElementType::Int64, 2),
        ("marker_pos", TensorElementType::Int64, 2),
        ("marker_mask", TensorElementType::Bool, 2),
        ("qtype", TensorElementType::Int64, 1),
    ];
    if session.inputs().len() != required.len() {
        return Err(Error::InvalidModelInput(format!(
            "expected five graph inputs, got {}",
            session.inputs().len()
        )));
    }
    for (name, ty, rank) in required {
        let input = session
            .inputs()
            .iter()
            .find(|input| input.name() == name)
            .ok_or_else(|| Error::InvalidModelInput(format!("missing graph input {name}")))?;
        if input.dtype().tensor_type() != Some(ty)
            || input.dtype().tensor_shape().is_none_or(|s| s.len() != rank)
        {
            return Err(Error::InvalidModelInput(format!(
                "invalid graph input {name}: {}",
                input.dtype()
            )));
        }
    }
    for (name, rank) in [("logits", 2), ("act_logits", 2), ("last_hidden_state", 3)] {
        let output = session
            .outputs()
            .iter()
            .find(|output| output.name() == name)
            .ok_or_else(|| Error::InvalidModelInput(format!("missing graph output {name}")))?;
        if output.dtype().tensor_type() != Some(TensorElementType::Float32)
            || output
                .dtype()
                .tensor_shape()
                .is_none_or(|s| s.len() != rank)
        {
            return Err(Error::InvalidModelInput(format!(
                "invalid graph output {name}: {}",
                output.dtype()
            )));
        }
    }
    Ok(())
}

fn clamp_temperature(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0.5, 5.0)
    } else {
        1.0
    }
}
fn round4(value: f64) -> f64 {
    (value * 10_000.0).round() / 10_000.0
}
fn entropy_confidence(probs: &[f64]) -> f64 {
    if probs.len() < 2 {
        return 1.0;
    }
    let entropy: f64 = probs
        .iter()
        .filter(|p| **p > 0.0)
        .map(|p| -p * p.ln())
        .sum();
    (1.0 - entropy / (probs.len() as f64).ln()).clamp(0.0, 1.0)
}
fn softmax(logits: &[f32], temperature: f64) -> Result<Vec<f64>> {
    if logits.is_empty() || logits.iter().any(|v| !v.is_finite()) {
        return Err(Error::OutputDecode(
            "empty or non-finite model logits".into(),
        ));
    }
    let max = logits.iter().copied().fold(f32::NEG_INFINITY, f32::max) as f64;
    let mut values: Vec<f64> = logits
        .iter()
        .map(|v| ((f64::from(*v) - max) / temperature).exp())
        .collect();
    let sum: f64 = values.iter().sum();
    if !sum.is_finite() || sum == 0.0 {
        return Err(Error::OutputDecode("invalid logits normalization".into()));
    }
    for value in &mut values {
        *value /= sum;
    }
    Ok(values)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calibration_is_stable_and_type_independent() {
        let p = softmax(&[2.0, 2.0], 1.0).unwrap();
        assert_eq!(p, [0.5, 0.5]);
        assert_eq!(entropy_confidence(&p), 0.0);
        let sharp = softmax(&[2.0, 0.0], clamp_temperature(0.01)).unwrap();
        let soft = softmax(&[2.0, 0.0], clamp_temperature(100.0)).unwrap();
        assert!(sharp[0] > soft[0]);
        assert_eq!(clamp_temperature(f64::NAN), 1.0);
        assert_eq!(entropy_confidence(&[1.0]), 1.0);
        assert!(softmax(&[f32::NAN], 1.0).is_err());
    }
    #[test]
    fn checkpoint_temperature_buckets_override_type_and_clamp_unsafe_values() {
        let config: Config = serde_json::from_str(
            r#"{"temperature":[1.6369030475616455,1.2514300346374512,1.983399510383606],
                 "temperature_by_options":{"choice:2":1.9063563346862793,"choice:3-5":1.7601518630981445,
                    "choice:11+":0.10058280825614929,"noul:2":1.983399510383606}}"#
        ).unwrap();
        assert!((temperature_for(&config, "choice", 2) - 1.9063563346862793).abs() < 1e-12);
        assert!((temperature_for(&config, "choice", 4) - 1.7601518630981445).abs() < 1e-12);
        assert_eq!(temperature_for(&config, "choice", 12), 0.5);
        assert!((temperature_for(&config, "score", 4) - 1.2514300346374512).abs() < 1e-12);
        assert!((temperature_for(&config, "noul", 2) - 1.983399510383606).abs() < 1e-12);
        let distribution = softmax(&[0.0, 2.0], temperature_for(&config, "choice", 2)).unwrap();
        assert!((distribution[1] - 0.7405).abs() < 0.001);
    }
    #[test]
    fn sequence_positions_and_state_budget_preserve_all_options() {
        let special = Special {
            cls: 101,
            sep: 102,
            mask: 103,
            pad: 0,
        };
        let row = assemble_row(
            special,
            vec![7, 8],
            vec![vec![103, 11], vec![103, 12, 13]],
            &[21, 22, 23],
            12,
            "example",
            0,
        )
        .unwrap();
        assert_eq!(row.markers, [4, 6]);
        assert_eq!(
            row.ids,
            [101, 7, 8, 102, 103, 11, 103, 12, 13, 102, 21, 102]
        );
        assert!(
            assemble_row(
                special,
                vec![7, 8],
                vec![vec![103, 11], vec![103, 12, 13]],
                &[],
                9,
                "example",
                0
            )
            .is_err()
        );
    }
}
