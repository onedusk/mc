# /explore-react-style — React → Hydrogen style exploration (JSON-first)

## Goal
Explore + index a **source React project** (design patterns, Tailwind/tokens, component inventory) and a **target Shopify Hydrogen store** (routes/entrypoints, GraphQL usage, dependency graph) so we can:
- combine/port styles and components safely
- answer “what breaks if I change X?” using cached indexes (no full rescan unless files changed)

## What I want you (the agent) to do
You will:
1) Confirm **SOURCE_REPO** (React repo) and **TARGET_REPO** (Hydrogen repo).
   - If I’m currently inside the Hydrogen repo in Cursor, assume **TARGET_REPO = current workspace**.
   - Ask me for SOURCE_REPO path if not provided.
2) Create a script at: `./.agent-tools/explore-react-style.sh` inside TARGET_REPO (so it’s project-local).
3) Run it to generate outputs in: `./.agent-docs/react-style/`
4) Summarize findings by pointing me to:
   - `views/STYLE_TRANSFER_PLAN.md`
   - `views/GRAPHQL_USAGE.md`
   - `graphs/imports.target.mmd`
   - `graphs/graphql-usage.target.mmd`

## Inputs (ask me if missing)
- SOURCE_REPO: absolute or relative path
- TARGET_REPO: absolute or relative path (default: current repo)
- Optional: `CHANGED_SINCE` git ref (e.g., `main`) to generate impact-only info

## Output Contract (must match exactly)
Write all artifacts under:
`TARGET_REPO/.agent-docs/react-style/`

- `manifest.source.json`
- `manifest.target.json`

- `indexes/`
  - `imports.source.json`
  - `imports.target.json`
  - `tailwind.source.json`
  - `tailwind.target.json`
  - `components.source.json`
  - `components.target.json`
  - `routes.target.json`
  - `graphql.target.json`
  - `style_transfer_map.json`
  - `impact.target.json` (ONLY if CHANGED_SINCE provided)

- `graphs/`
  - `imports.target.dot`
  - `imports.target.mmd`
  - `graphql-usage.target.mmd`
  - `style-transfer.mmd` (optional but recommended)

- `views/`
  - `PROJECT_SUMMARY.md`
  - `SOURCE_STYLE_PROFILE.md`
  - `TARGET_STYLE_PROFILE.md`
  - `GRAPHQL_USAGE.md`
  - `STYLE_TRANSFER_PLAN.md`
  - `IMPACT_REPORT.md` (ONLY if CHANGED_SINCE provided)
  - `WORKING_NOTES.md`

## Script to create (copy exactly)
Create `TARGET_REPO/.agent-tools/explore-react-style.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

SOURCE="${1:?source repo path required}"
TARGET="${2:?target repo path required}"

OUT="${3:-.agent-docs/react-style}"
CACHE="${4:-$OUT/.cache}"

TIER="${TIER:-1}"                 # reserved for future semantic tiering
CHANGED_SINCE="${CHANGED_SINCE:-}"
MAX_FILES="${MAX_FILES:-20000}"
EXCLUDE="${EXCLUDE:-node_modules,dist,build,.next,.cache,.git,coverage,.turbo,.vercel,.output,public/build}"

mkdir -p "$OUT/indexes" "$OUT/graphs" "$OUT/views" "$CACHE" "$TARGET/.agent-tools"

need() { command -v "$1" >/dev/null 2>&1 || { echo "Missing required tool: $1" >&2; exit 1; }; }
need jq
need rg
need python3
need file

IFS=',' read -r -a EXCLUDES <<< "$EXCLUDE"

build_rg_globs() {
  local globs=()
  for ex in "${EXCLUDES[@]}"; do
    globs+=( "--glob" "!**/${ex}/**" )
  done
  printf "%s " "${globs[@]}"
}
RG_GLOBS="$(build_rg_globs)"

hash_file() {
  local f="$1"
  if command -v sha1sum >/dev/null 2>&1; then
    sha1sum "$f" | awk '{print $1}'
  else
    shasum -a 1 "$f" | awk '{print $1}'
  fi
}

mime_is_text() {
  local f="$1"
  file -b --mime "$f" 2>/dev/null | grep -qi 'charset=binary' && return 1
  return 0
}

scan_manifest() {
  local ROOT="$1"
  local OUTFILE="$2"
  local TMP="$CACHE/manifest.$(basename "$OUTFILE").jsonl"
  : > "$TMP"

  mapfile -t files < <(rg -uuu --files "$ROOT" $RG_GLOBS)

  local count="${#files[@]}"
  if (( count > MAX_FILES )); then
    echo "Too many files ($count) > max-files ($MAX_FILES). Narrow scope or increase MAX_FILES." >&2
    exit 1
  fi

  for f in "${files[@]}"; do
    mime_is_text "$f" || continue
    local ext="${f##*.}"
    local size
    size="$(wc -c < "$f" | tr -d ' ')"
    local hash
    hash="$(hash_file "$f")"
    printf '{"path":%q,"ext":%q,"size":%s,"hash":%q}\n' "$f" "$ext" "$size" "$hash" >> "$TMP"
  done

  jq -s --arg root "$ROOT" '{root:$root, generated_at:(now|floor), files:.}' "$TMP" > "$OUTFILE"
}

extract_imports_js_ts() {
  local ROOT="$1"
  local OUTFILE="$2"

  rg -n --no-messages -S \
    -e '^\s*import\s+.*from\s+["'\'']' \
    -e '^\s*import\s+["'\'']' \
    -e '^\s*export\s+\*\s+from\s+["'\'']' \
    -e '^\s*const\s+.*=\s*require\(' \
    -e '^\s*require\(["'\'']' \
    "$ROOT" $RG_GLOBS \
    --glob '**/*.{ts,tsx,js,jsx}' \
  | jq -Rn '
      [inputs
        | capture("^(?<file>[^:]+):(?<line>\\d+):(?<text>.*)$")
        | .line |= (tonumber)
      ]' > "$OUTFILE" || echo "[]" > "$OUTFILE"
}

extract_tailwind() {
  local ROOT="$1"
  local OUTFILE="$2"

  local cfg
  cfg="$(rg -uuu --files "$ROOT" $RG_GLOBS | rg -n --no-messages 'tailwind\.config\.(js|cjs|ts|mjs)$|postcss\.config\.(js|cjs|ts|mjs)$|app/styles/tailwind\.css|src/styles/tailwind\.css' || true)"

  # Sample class usage (bounded)
  local class_hits
  class_hits="$(rg -n --no-messages -S 'className\s*=\s*["'\'']|tw`|clsx\(|\bcn\(' "$ROOT" $RG_GLOBS --glob '**/*.{ts,tsx,js,jsx}' | head -5000 || true)"

  jq -n --arg root "$ROOT" \
    --arg config_files "$cfg" \
    --arg sample_class_usages "$class_hits" \
    '{
      root:$root,
      tailwind_config_candidates: ($config_files|split("\n")|map(select(length>0))),
      class_usage_sample: ($sample_class_usages|split("\n")|map(select(length>0)))
    }' > "$OUTFILE"
}

extract_components_heuristic() {
  local ROOT="$1"
  local OUTFILE="$2"

  rg -n --no-messages -S \
    -e 'export\s+default\s+function\s+[A-Z][A-Za-z0-9_]+' \
    -e 'export\s+function\s+[A-Z][A-Za-z0-9_]+' \
    -e 'const\s+[A-Z][A-Za-z0-9_]+\s*=\s*\(?.*\)?\s*=>\s*\(' \
    -e ':\s*React\.FC' \
    -e 'return\s+\(\s*<\w+' \
    "$ROOT" $RG_GLOBS \
    --glob '**/*.{ts,tsx,js,jsx}' \
  | jq -Rn '
      [inputs
        | capture("^(?<file>[^:]+):(?<line>\\d+):(?<text>.*)$")
        | .line |= (tonumber)
      ]' > "$OUTFILE" || echo "[]" > "$OUTFILE"
}

extract_hydrogen_routes() {
  local ROOT="$1"
  local OUTFILE="$2"

  local routes
  routes="$(rg -uuu --files "$ROOT" $RG_GLOBS | rg '^.*/app/routes/.*\.(tsx|ts|jsx|js)$' || true)"

  local entry
  entry="$(rg -uuu --files "$ROOT" $RG_GLOBS | rg '(^|/)(app/root\.(tsx|ts|jsx|js)|app/entry\.server\.(tsx|ts|jsx|js)|server\.(ts|js)|vite\.config\.(ts|js))$' || true)"

  jq -n --arg root "$ROOT" \
    --arg routes "$routes" \
    --arg entrypoints "$entry" \
    '{
      root:$root,
      remix_routes: ($routes|split("\n")|map(select(length>0))),
      entrypoints: ($entrypoints|split("\n")|map(select(length>0)))
    }' > "$OUTFILE"
}

extract_graphql_hydrogen() {
  local ROOT="$1"
  local OUTFILE="$2"

  local tmp_hits="$CACHE/graphql_hits.txt"
  : > "$tmp_hits"

  rg -n --no-messages -S \
    -e 'storefront\.query\s*\(' \
    -e 'storefront\.mutate\s*\(' \
    -e '\bgraphql`' \
    -e '\bgql`' \
    -e '^\s*#graphql\b' \
    -e '@shopify\/hydrogen' \
    -e '@shopify\/remix-oxygen' \
    "$ROOT" $RG_GLOBS \
    --glob '**/*.{ts,tsx,js,jsx}' \
    > "$tmp_hits" || true

  python3 - <<'PY' "$tmp_hits" "$OUTFILE" "$ROOT"
import json, re, sys
hits_path, out_path, root = sys.argv[1], sys.argv[2], sys.argv[3]

hits = []
ops = {}
frags = {}

op_re = re.compile(r'\b(query|mutation|subscription)\s+([A-Za-z_][A-Za-z0-9_]*)')
frag_re = re.compile(r'\bfragment\s+([A-Za-z_][A-Za-z0-9_]*)\s+on\b')

with open(hits_path, "r", encoding="utf-8", errors="replace") as f:
    for line in f:
        line=line.rstrip("\n")
        m = re.match(r'^(?P<file>[^:]+):(?P<ln>\d+):(?P<text>.*)$', line)
        if not m:
            continue
        file = m.group("file")
        ln = int(m.group("ln"))
        text = m.group("text")

        hits.append({"file": file, "line": ln, "text": text})

        for mm in op_re.finditer(text):
            kind, name = mm.group(1), mm.group(2)
            ops.setdefault(name, {"kind": kind, "locations": []})
            ops[name]["locations"].append({"file": file, "line": ln})

        for mm in frag_re.finditer(text):
            name = mm.group(1)
            frags.setdefault(name, {"locations": []})
            frags[name]["locations"].append({"file": file, "line": ln})

out = {
    "root": root,
    "hits": hits,
    "operations": ops,
    "fragments": frags,
    "notes": [
        "Heuristic (line-based). For higher accuracy, add semantic parsing to extract full gql documents.",
        "Hydrogen Storefront API usage commonly appears as storefront.query(...) with embedded GraphQL."
    ]
}
with open(out_path, "w", encoding="utf-8") as w:
    json.dump(out, w, indent=2)
PY
}

generate_import_graphs() {
  local IMPORTS_JSON="$1"
  local DOT_OUT="$2"
  local MMD_OUT="$3"

  python3 - <<'PY' "$IMPORTS_JSON" "$DOT_OUT" "$MMD_OUT"
import json, re, sys, os
imports_path, dot_path, mmd_path = sys.argv[1], sys.argv[2], sys.argv[3]
data = json.load(open(imports_path))
edges = []
for row in data:
    f = row.get("file","")
    t = row.get("text","")
    m = re.search(r'from\s+[\'"]([^\'"]+)[\'"]', t) or re.search(r'import\s+[\'"]([^\'"]+)[\'"]', t)
    if m:
        edges.append((f, m.group(1)))

with open(dot_path,"w") as w:
    w.write("digraph imports {\n")
    for a,b in edges[:3000]:
        w.write(f"  \"{a}\" -> \"{b}\";\n")
    w.write("}\n")

def nid(s): return str(abs(hash(s))%(10**9))
with open(mmd_path,"w") as w:
    w.write("graph TD\n")
    for a,b in edges[:3000]:
        w.write(f"  {nid(a)}[\"{os.path.basename(a)}\"] --> {nid(b)}[\"{b}\"]\n")
PY
}

generate_graphql_usage_graph() {
  local GRAPHQL_JSON="$1"
  local ROUTES_JSON="$2"
  local MMD_OUT="$3"

  python3 - <<'PY' "$GRAPHQL_JSON" "$ROUTES_JSON" "$MMD_OUT"
import json, sys, os
gq_path, routes_path, out_path = sys.argv[1], sys.argv[2], sys.argv[3]
gq = json.load(open(gq_path))
routes = json.load(open(routes_path))

route_files = set(routes.get("remix_routes", []))
entry_files = set(routes.get("entrypoints", []))

def role(f):
  if f in entry_files: return "entry"
  if f in route_files: return "route"
  return "module"

by_file = {}
for h in gq.get("hits", []):
  f = h.get("file")
  if not f: continue
  by_file[f] = by_file.get(f, 0) + 1

top = sorted(by_file.items(), key=lambda x: -x[1])[:80]

def nid(s): return str(abs(hash(s))%(10**9))

with open(out_path, "w") as w:
  w.write("graph TD\n")
  w.write("  H[\"Hydrogen Storefront GraphQL\"]\n")
  for f, n in top:
    label = f"{os.path.basename(f)}\\\\n({role(f)}, hits:{n})"
    w.write(f"  {nid(f)}[\"{label}\"]\n")
    w.write(f"  H --> {nid(f)}\n")
PY
}

generate_style_transfer_map() {
  local SRC_TW="$1"
  local TGT_TW="$2"
  local SRC_COMP="$3"
  local TGT_ROUTES="$4"
  local OUTFILE="$5"

  python3 - <<'PY' "$SRC_TW" "$TGT_TW" "$SRC_COMP" "$TGT_ROUTES" "$OUTFILE"
import json, sys, os
src_tw = json.load(open(sys.argv[1]))
tgt_tw = json.load(open(sys.argv[2]))
src_comp = json.load(open(sys.argv[3]))
tgt_routes = json.load(open(sys.argv[4]))

def uniq_files(rows, limit=200):
  seen=set(); out=[]
  for r in rows:
    p=r.get("file")
    if p and p not in seen:
      seen.add(p); out.append(p)
    if len(out)>=limit: break
  return out

target_entry = tgt_routes.get("entrypoints", [])
target_routes = tgt_routes.get("remix_routes", [])

mapping = {
  "theme_transfer": {
    "source_tailwind_candidates": src_tw.get("tailwind_config_candidates", []),
    "target_tailwind_candidates": tgt_tw.get("tailwind_config_candidates", []),
    "recommended_target_touchpoints": [
      *[p for p in target_entry if "app/root." in p or "tailwind" in p],
      *[p for p in target_entry if "entry.server" in p],
    ]
  },
  "component_transfer": {
    "source_component_files_sample": uniq_files(src_comp, limit=200),
    "target_insertion_points": {
      "layouts_and_root": [p for p in target_entry if "app/root." in p],
      "route_files_sample": target_routes[:40],
    },
    "notes": [
      "Port tokens/theme first (Tailwind config, CSS vars, global CSS) before moving components.",
      "Standardize cn()/clsx() + class conventions early to avoid mismatch churn.",
      "Hydrogen global styling usually funnels through app/root and app/styles/*."
    ]
  }
}

json.dump(mapping, open(sys.argv[5], "w"), indent=2)
PY
}

run_target_impact() {
  local ROOT="$1"
  local GITREF="$2"
  local OUTFILE="$3"

  (cd "$ROOT" && git rev-parse --is-inside-work-tree >/dev/null 2>&1) || {
    echo "Impact mode requires TARGET to be a git repo." >&2
    return 0
  }

  local changed
  changed="$(cd "$ROOT" && git diff --name-only "$GITREF"...HEAD || true)"

  jq -n --arg changed_since "$GITREF" --arg changed "$changed" \
    '{changed_since:$changed_since, files: ($changed|split("\n")|map(select(length>0)))}' > "$OUTFILE"
}

# -------------------------------
# Execute
# -------------------------------
echo "== manifests =="
scan_manifest "$SOURCE" "$OUT/manifest.source.json"
scan_manifest "$TARGET" "$OUT/manifest.target.json"

echo "== indexes =="
extract_imports_js_ts "$SOURCE" "$OUT/indexes/imports.source.json"
extract_imports_js_ts "$TARGET" "$OUT/indexes/imports.target.json"

extract_tailwind "$SOURCE" "$OUT/indexes/tailwind.source.json"
extract_tailwind "$TARGET" "$OUT/indexes/tailwind.target.json"

extract_components_heuristic "$SOURCE" "$OUT/indexes/components.source.json"
extract_components_heuristic "$TARGET" "$OUT/indexes/components.target.json"

extract_hydrogen_routes "$TARGET" "$OUT/indexes/routes.target.json"
extract_graphql_hydrogen "$TARGET" "$OUT/indexes/graphql.target.json"

generate_style_transfer_map \
  "$OUT/indexes/tailwind.source.json" \
  "$OUT/indexes/tailwind.target.json" \
  "$OUT/indexes/components.source.json" \
  "$OUT/indexes/routes.target.json" \
  "$OUT/indexes/style_transfer_map.json"

if [[ -n "$CHANGED_SINCE" ]]; then
  run_target_impact "$TARGET" "$CHANGED_SINCE" "$OUT/indexes/impact.target.json"
fi

echo "== graphs =="
generate_import_graphs \
  "$OUT/indexes/imports.target.json" \
  "$OUT/graphs/imports.target.dot" \
  "$OUT/graphs/imports.target.mmd"

generate_graphql_usage_graph \
  "$OUT/indexes/graphql.target.json" \
  "$OUT/indexes/routes.target.json" \
  "$OUT/graphs/graphql-usage.target.mmd"

echo "== views =="
cat > "$OUT/views/PROJECT_SUMMARY.md" <<EOF
# Project Summary

## Source
- Root: $(jq -r .root "$OUT/manifest.source.json")
- Files indexed: $(jq -r '.files|length' "$OUT/manifest.source.json")

## Target (Hydrogen)
- Root: $(jq -r .root "$OUT/manifest.target.json")
- Files indexed: $(jq -r '.files|length' "$OUT/manifest.target.json")
EOF

jq -r '
  "# Source Style Profile\n\n" +
  "## Tailwind Config Candidates\n" +
  (.tailwind_config_candidates|map("- " + .)|join("\n")) + "\n\n" +
  "## Component Signals (sample)\n" +
  ($components[0:50] | map("- " + .file + ":" + (.line|tostring)) | join("\n")) + "\n"
' "$OUT/indexes/tailwind.source.json" \
  --argjson components "$(cat "$OUT/indexes/components.source.json")" \
  > "$OUT/views/SOURCE_STYLE_PROFILE.md"

jq -r '
  "# Target Style Profile (Hydrogen)\n\n" +
  "## Entrypoints\n" +
  (.entrypoints|map("- " + .)|join("\n")) + "\n\n" +
  "## Remix Routes (sample)\n" +
  (.remix_routes[0:50]|map("- " + .)|join("\n")) + "\n"
' "$OUT/indexes/routes.target.json" > "$OUT/views/TARGET_STYLE_PROFILE.md"

jq -r '
  "# GraphQL Usage (Target Hydrogen)\n\n" +
  "## Top callsites (sample)\n" +
  ( .hits[0:120]
    | map("- " + .file + ":" + (.line|tostring) + "  " + (.text|gsub("\\s+";" ")|.[0:140]))
    | join("\n")
  ) + "\n\n" +
  "## Operations detected (heuristic)\n" +
  ( ( .operations | to_entries | sort_by(.key) )
    | map("- " + .value.kind + " " + .key + " (" + ((.value.locations|length)|tostring) + " refs)")
    | join("\n")
  ) + "\n\n" +
  "## Fragments detected (heuristic)\n" +
  ( ( .fragments | to_entries | sort_by(.key) )
    | map("- fragment " + .key + " (" + ((.value.locations|length)|tostring) + " refs)")
    | join("\n")
  ) + "\n"
' "$OUT/indexes/graphql.target.json" > "$OUT/views/GRAPHQL_USAGE.md"

jq -r '
  "# Style Transfer Plan\n\n" +
  "## Theme / Tokens\n" +
  "- Source Tailwind candidates:\n" + (.theme_transfer.source_tailwind_candidates|map("  - " + .)|join("\n")) + "\n" +
  "- Target Tailwind candidates:\n" + (.theme_transfer.target_tailwind_candidates|map("  - " + .)|join("\n")) + "\n" +
  "- Recommended target touchpoints:\n" + (.theme_transfer.recommended_target_touchpoints|map("  - " + .)|join("\n")) + "\n\n" +
  "## Components\n" +
  "- Source component files (sample):\n" + (.component_transfer.source_component_files_sample[0:60]|map("  - " + .)|join("\n")) + "\n" +
  "- Target insertion points:\n" +
  "  - Root/layout:\n" + (.component_transfer.target_insertion_points.layouts_and_root|map("    - " + .)|join("\n")) + "\n" +
  "  - Routes (sample):\n" + (.component_transfer.target_insertion_points.route_files_sample|map("    - " + .)|join("\n")) + "\n\n" +
  "## Notes\n" + (.component_transfer.notes|map("- " + .)|join("\n")) + "\n"
' "$OUT/indexes/style_transfer_map.json" > "$OUT/views/STYLE_TRANSFER_PLAN.md"

if [[ -n "$CHANGED_SINCE" ]]; then
  jq -r '
    "# Impact Report\n\n" +
    "Changed since: \(.changed_since)\n\n" +
    "## Files\n" + (.files|map("- " + .)|join("\n")) + "\n"
  ' "$OUT/indexes/impact.target.json" > "$OUT/views/IMPACT_REPORT.md"
fi

cat > "$OUT/views/WORKING_NOTES.md" <<'MD'
# Working Notes

## Best order for style transfer
1) Align **tokens/theme** (Tailwind config, CSS vars, global CSS).
2) Standardize utilities (`cn`/`clsx`) and class conventions.
3) Port foundational components (Button/Input/Card/Modal).
4) Port page sections last (Hero/ProductGrid/Filters).

## Hydrogen global touchpoints
- app/root.*
- app/entry.server.*
- app/routes/**
- app/styles/**
MD

echo "✅ Done. Output: $OUT"
