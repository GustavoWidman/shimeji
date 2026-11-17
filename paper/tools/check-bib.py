import json
import re
import sys
import time
import unicodedata
import urllib.request

bib = sys.argv[1] if len(sys.argv) > 1 else "paper/refs.bib"
text = open(bib, encoding="utf-8").read()


def norm(s):
    s = unicodedata.normalize("NFKD", s)
    s = re.sub(r"[{}\\]|\$[^$]*\$", "", s)
    return re.sub(r"[^a-z0-9]+", " ", s.lower()).strip()


def field(body, name):
    m = re.search(rf"\b{name}\s*=\s*\{{((?:[^{{}}]|\{{[^{{}}]*\}})*)\}}", body, re.S)
    return m.group(1) if m else None


failed = 0
for key, body in re.findall(r"@\w+\{([^,]+),(.*?)\n\}", text, re.S):
    doi, title, year = field(body, "doi"), field(body, "title"), field(body, "year")
    if not doi:
        print(f"no doi  {key}")
        continue
    req = urllib.request.Request(
        f"https://api.crossref.org/works/{doi}", headers={"User-Agent": "bib-check/0.1"}
    )
    try:
        work = json.load(urllib.request.urlopen(req, timeout=30))["message"]
    except Exception as e:
        print(f"FAIL    {key}: {doi}: {e}")
        failed += 1
        continue
    got_title = (work.get("title") or [""])[0]
    parts = (work.get("published-print") or work.get("published-online") or work.get("issued"))["date-parts"][0]
    got_year = str(parts[0])
    title_ok = norm(got_title) == norm(title or "")
    year_ok = got_year == year
    if title_ok and year_ok:
        print(f"ok      {key}")
    elif year_ok and norm(got_title).startswith(norm(title or "")):
        print(f"ok      {key} (crossref title runs on: '{got_title}')")
    else:
        failed += 1
        print(f"FAIL    {key}: crossref has '{got_title}' ({got_year}); entry has '{title}' ({year})")
    time.sleep(0.5)
sys.exit(1 if failed else 0)
