# Local System-1 judge example

The model judges a task output against a typed question; a low-confidence result is escalated to an LLM judge. This is an integration mechanism, not a recommended numeric threshold. Select and validate the threshold on your actual task distribution; `answer_confidence` is the relevant top-answer probability rather than entropy `confidence`.

```python
# This orchestration example uses Python, but laya-rs itself does not require Python.
import json, subprocess

def judge(task_output: str, threshold: float, llm_judge):
    request = {
        "state": task_output,
        "questions": {"acceptable": {
            "type": "noul",
            "instructions": "Does this output meet the task's acceptance criteria?"
        }}
    }
    run = subprocess.run(
        ["laya", "--device", "cpu", "predict", "-"],
        input=json.dumps(request), text=True, capture_output=True, check=True,
    )
    answer = json.loads(run.stdout)["acceptable"]
    if answer["answer_confidence"] < threshold:
        return llm_judge(task_output)
    return answer["value"]
```

Keep stderr separate from stdout JSON. For a long-lived judge service, hold a `Laya` instance in the host process and call `predict` repeatedly; the CLI starts a new process/session per invocation.
