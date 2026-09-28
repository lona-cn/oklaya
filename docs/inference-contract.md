# Laya inference contract

Source: [official `common.py`](https://github.com/NandhaKishorM/laya/blob/9d955671415fc19f069b9cc998928075c1f255ec/laya/common.py), [official ONNX runtime](https://github.com/NandhaKishorM/laya/blob/9d955671415fc19f069b9cc998928075c1f255ec/laya/onnx_agent.py). Graph below inspected directly from the selected multilingual ONNX (PyTorch 2.14.0, opset 20, 1494 nodes); do not substitute the older official exporter example (opset 18, two outputs) for this graph.

## Graph

| Direction | Name | Type | Shape | Meaning |
|---|---|---|---|---|
| input | `input_ids` | int64 | `[batch, seq]` | padded token IDs |
| input | `attention_mask` | int64 | `[batch, seq]` | 1 on real tokens, 0 on padding |
| input | `marker_pos` | int64 | `[batch, markers]` | absolute `[MASK]` positions; padding 0 |
| input | `marker_mask` | bool | `[batch, markers]` | true for real options |
| input | `qtype` | int64 | `[batch]` | choice=0, score=1, noul=2 |
| output | `logits` | float32 | `[batch, markers]` | raw, masked option logits |
| output | `act_logits` | float32 | `[batch, 2]` | independent action logits (index 0 probability is `act_probability`) |
| output | `last_hidden_state` | float32 | `[batch, seq, 768]` | optional embedding output; not required for decisions |

All symbolic dimensions `batch`, `seq`, `markers` are dynamic; the selected export requires at least 8 sequence tokens and 2 marker slots, padding a single-option row with a masked second slot. It contains encoder **and** typed decision/action heads. Request only the first two outputs where possible to avoid the embedding output.

The official Python ONNX wrapper fails with `TopK k=2` on a single-option row of this conversion; the Rust runtime pads marker slots to two and masks the second slot. A real single-option CPU inference returned that option with probability 1.0. This behavior has no valid single-option Python ONNX oracle for comparison.

## Preprocessing

The official Python runtime accepts a string, dict or conversation list as state (serializing structured values to JSON and retaining the suffix for conversation lists). This Rust CLI currently accepts **string state only**; callers must serialize structured state explicitly and long strings retain the prefix. Questions preserve insertion order; each becomes one batch row, with shared state tokenized once. `tokenizer/tokenizer.json` is loaded by the Hugging Face tokenizer; **disable automatic special tokens** for segments. The tokenizer's CLS, SEP, MASK and PAD IDs come from the artifact rather than hardcoded constants. Replace literal mask-token text in user fields with a space.

Each row: `[CLS] <kind> question: <instructions> [SEP] [MASK] <option0> [MASK] <option1> ... [SEP] <state> [SEP]`. Encode question without special tokens. Encode each option prefixed with a space without special tokens, truncate each option at 48 tokens, prepend MASK and record its absolute position. Choice option text is `label` for an empty description, else `label: description`. Score option text is `level 0: criterion`, `level 1: criterion`, etc. Noul options are always `[false: no, the statement does not hold, true: yes, the statement holds]` unless supplied descriptions override them. The second option is P(true).

Compute `opt_budget = head_max_len - sum(option lengths including marker)`; if below 16, truncate each option to `max(4, (head_max_len-16)/option_count)` tokens then recompute. Truncate question head to `max(8, opt_budget)` tokens. Append available state prefix until `max_len` (for conversation lists, keep suffix); terminate with SEP. The checkpoint config determines default limits (multilingual 1024/256; English and specialist differ). Disclose state truncation; reject options whose markers cannot fit. Pad rows to max sequence/marker lengths; use attention=0, marker_mask=false for padding.

## Decode / calibration

Read `temperature[choice,score,noul]` and optional `temperature_by_options["kind:size"]` from `rl_agent_config.json`. Bucket is `2` for k≤2, `3-5` for 3–5, `6-10` for 6–10, `11+` otherwise. Temperatures are clamped to [0.5,5.0], invalid entries become 1.0. Compute softmax of `logits[:k] / temperature`, subtracting row max for stability; masked slots excluded.

Choice: argmax selects the positional original label; probabilities per option. Score: expected *zero-based index* `sum(i*p_i)` with per-level probabilities; not an ordinal label. Noul/bool: `p[1]` is P(true), boolean decision uses `p[1] >= 0.5`. All official published probabilities and scores are rounded to 4 decimal places. The upstream `confidence` for choice/score is normalized entropy `1-H(p)/ln(k)`, **not** calibrated correctness probability. `answer_confidence=max(p)` is the temperature-calibrated top-answer mass. Noul `confidence=max(p[1],1-p[1])`; its `answer_confidence` is also max(p). Action probability is softmax(act_logits)[0], separate from question probabilities. Thresholds must be calibrated in the deployment domain.

A single `Session::run` handles all question rows for one state. Reusing one model session and tokenizer across requests avoids repeated startup. The official `Router` chooses a checkpoint from language/schema heuristics; selecting the multilingual checkpoint explicitly avoids silently using the wrong model on non-English requests.
