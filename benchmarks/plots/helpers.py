import os
import re

__dirname = os.path.dirname(__file__)

ROOT_DIR = os.path.dirname(__dirname)
DATA_DIR = os.path.join(ROOT_DIR, "data")

PROJECT_NAMES = {
    "cheerpj": "CheerpJ",
    "gwt": "GWT",
    "handwritten": "Handwritten WASM",
    "javascript": "Handwritten JavaScript",
    "jvm": "JVM",
    "jwebassembly": "JWebAssembly",
    "montera": "My Project",
    "monteraopt": "My Project (Optimised)",
    "teavm": "TeaVM"
}

PROJECT_COLOURS = {
    "CheerpJ": "#ef8232",
    "GWT": "#e54840",
    "Handwritten WASM": "#6056e7",
    "Handwritten JavaScript": "#e5d365",
    "JVM": "#5c82a0",
    "JWebAssembly": "#205385",
    "My Project": "#ec407a",
    "My Project (Optimised)": "#ec407a",
    "TeaVM": "#00a52b",
}


def latest_output_path(name: str):
    r = re.compile(r"^" + name + r"(-\d+){6}.csv$")
    # Sort all matching entries with earlier outputs first
    matches = sorted((path for path in os.listdir(DATA_DIR) if r.match(path)))
    if len(matches) == 0:
        raise FileNotFoundError(f"No matching data file: '{name}'")
    # Return last (latest) output
    return os.path.join(DATA_DIR, matches[-1])
