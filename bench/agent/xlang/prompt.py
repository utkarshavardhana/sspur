"""Print the prompt of one cell: python3 prompt.py WORK_ROOT LANG TASK"""
import json, os, sys

root, lang, task = sys.argv[1:4]
for p in json.load(open(os.path.join(root, "prompts.json"))):
    if p["lang"] == lang and p["task"] == task:
        print(p["prompt"])
