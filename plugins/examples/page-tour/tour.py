import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from systm_cut import app

pages = ["media", "cut", "edit", "fusion", "color", "fairlight", "depth", "deliver", "edit"]
for p in pages:
    app.call("ui.click", {"id": f"page.{p}"})
    time.sleep(1.0)
print(f"toured {len(pages) - 1} pages")
