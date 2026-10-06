"""Independent volume assertions on retained actual kernel-read evidence."""
import json
from pathlib import Path
root = Path(__file__).resolve().parent
old = json.loads((root / "frozen-reads.json").read_text())["cases"]
new = json.loads((root / "repaired-reads.json").read_text())["cases"]
for name in ("projection.json", "roots.in", "constraints.in"):
    before = next(r for r in old if r["file"] == name and r["growth"])
    after = next(r for r in new if r["file"] == name and r["growth"])
    assert before["largest_read_result"] == 32 * 1024 * 1024, name
    assert after["total_data_read_bytes"] < 2 * 1024 * 1024, name
    print(name, "frozen oversized read reproduced; repaired read volume bounded")
