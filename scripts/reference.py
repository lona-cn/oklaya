"""Development-only oracle; requires pip install 'laya[onnx]' (not required by Rust)."""
import argparse
import json
from pathlib import Path


def cases():
    states = [
        "The application crashes whenever I open settings.",
        "Please refund the duplicate payment.",
        "Cancel my subscription now.",
        "Production database is unavailable.",
        "La aplicación se cierra al abrir configuración.",
        "Veuillez annuler mon abonnement immédiatement.",
        "कृपया मेरा खाता रद्द करें।",
    ]
    for i, state in enumerate(states):
        yield {"id": f"choice-{i}", "state": state, "questions": {
            "department": {"type": "choice", "instructions": "Which department should handle this?",
                           "criteria": {"billing": "payments and refunds", "technical": "bugs and crashes", "other": "everything else"}}}}
        yield {"id": f"score-{i}", "state": state, "questions": {
            "urgency": {"type": "score", "instructions": "How urgent is this?", "criteria": ["low", "medium", "high"]}}}
        yield {"id": f"noul-{i}", "state": state, "questions": {
            "cancellation": {"type": "noul", "instructions": "Does the user want to cancel?"}}}
    yield {"id": "multiple-questions", "state": states[0], "questions": {
        "team": {"type": "choice", "instructions": "Which team?", "criteria": {"billing": "", "technical": ""}},
        "severity": {"type": "score", "instructions": "Severity?", "criteria": ["low", "medium", "high", "critical"]},
        "crash": {"type": "noul", "instructions": "Did the app crash?"}}}
    yield {"id": "six-options", "state": states[3], "questions": {
        "kind": {"type": "choice", "instructions": "Classify", "criteria": {
            "outage": "", "billing": "", "security": "", "feature": "", "support": "", "other": ""}}}}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    from laya.onnx_agent import ONNXAgent
    agent = ONNXAgent(str(args.model_dir), onnx_path=str(args.model_dir / "model.onnx"))
    output = []
    for case in cases():
        prediction = agent.predict(case["state"], case["questions"])
        output.append({**case, "answers": prediction["answers"]})
    args.output.write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
