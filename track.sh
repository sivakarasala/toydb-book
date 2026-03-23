#!/usr/bin/env bash
#
# toydb-book progress tracker
# Run: ./track.sh          (full check — runs cargo test on each chapter)
# Run: ./track.sh --quick  (fast check — skips tests, only checks file changes)
#
set -euo pipefail

BOOK_DIR="$(cd "$(dirname "$0")" && pwd)"
PROGRESS_FILE="$BOOK_DIR/.progress.json"
CODE_DIR="$BOOK_DIR/code"
QUICK=false

if [[ "${1:-}" == "--quick" ]]; then
    QUICK=true
fi

# ─── Colors ───────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
DIM='\033[2m'
RESET='\033[0m'

# ─── Progress file init ──────────────────────────────────────────
init_progress() {
    if [[ ! -f "$PROGRESS_FILE" ]]; then
        echo '{"exercises":{},"cumulative":{},"capstone":{},"activity_log":[]}' > "$PROGRESS_FILE"
    fi
}

# Read a JSON value (basic — no jq dependency)
read_json_array() {
    local key="$1"
    python3 -c "
import json, sys
with open('$PROGRESS_FILE') as f:
    data = json.load(f)
print(json.dumps(data.get('$key', {})))
" 2>/dev/null || echo "{}"
}

# Update progress file
update_progress() {
    python3 -c "
import json, sys
from datetime import date

with open('$PROGRESS_FILE') as f:
    data = json.load(f)

kind = sys.argv[1]  # exercises, cumulative, capstone
key = sys.argv[2]
status = sys.argv[3]  # pass, fail, active

if kind not in data:
    data[kind] = {}
data[kind][key] = status

# Log activity (deduplicate same day)
today = date.today().isoformat()
log = data.get('activity_log', [])
if not log or log[-1] != today:
    log.append(today)
data['activity_log'] = log[-365:]  # keep 1 year

with open('$PROGRESS_FILE', 'w') as f:
    json.dump(data, f, indent=2)
" "$1" "$2" "$3"
}

# ─── Check chapter exercises ─────────────────────────────────────
check_exercises() {
    local passed=0
    local failed=0
    local total=0
    local results=()

    for ch_num in $(seq -w 1 18); do
        local ch_dir="$CODE_DIR/ch${ch_num}"
        if [[ ! -d "$ch_dir" ]]; then
            continue
        fi
        total=$((total + 1))

        if $QUICK; then
            # Quick mode: check if exercise file has been modified (no more todo!())
            local main_file="$ch_dir/src/main.rs"
            if [[ -f "$main_file" ]] && ! grep -q 'todo!()' "$main_file" 2>/dev/null; then
                passed=$((passed + 1))
                results+=("${GREEN}  ✓ Ch ${ch_num}${RESET}")
                update_progress "exercises" "ch${ch_num}" "pass"
            else
                results+=("${DIM}  · Ch ${ch_num}${RESET}")
            fi
        else
            # Full mode: run cargo test
            if (cd "$ch_dir" && cargo test --bin exercise 2>&1 | grep -q "test result: ok"); then
                passed=$((passed + 1))
                results+=("${GREEN}  ✓ Ch ${ch_num}${RESET}")
                update_progress "exercises" "ch${ch_num}" "pass"
            else
                failed=$((failed + 1))
                results+=("${DIM}  · Ch ${ch_num}${RESET}")
            fi
        fi
    done

    echo -e "\n${BOLD}Chapter Exercises${RESET}  ${passed}/${total}"
    draw_bar "$passed" "$total"
    for r in "${results[@]}"; do
        echo -e "$r"
    done
}

# ─── Check cumulative project (my-toydb) ─────────────────────────
check_cumulative() {
    local lib_file="$CODE_DIR/my-toydb/src/lib.rs"
    if [[ ! -f "$lib_file" ]]; then
        echo -e "\n${BOLD}Cumulative Project${RESET}  (not started)"
        return
    fi

    local layers=(
        "storage:Storage Engine:Ch 1-2"
        "error:Error Types:Ch 3"
        "sql:SQL Pipeline:Ch 6-11"
        "raft:Raft Consensus:Ch 14-16"
    )

    # Also check individual files for TODO markers
    local detail_files=(
        "storage/mod.rs:Storage Trait:Ch 2"
        "storage/memory.rs:BTreeMap Engine:Ch 1-2"
        "error.rs:Error Types:Ch 3"
        "sql/lexer.rs:SQL Lexer:Ch 6"
        "sql/parser.rs:SQL Parser:Ch 7"
        "sql/planner.rs:Query Planner:Ch 8-9"
        "sql/executor.rs:Query Executor:Ch 10-11"
        "sql/types.rs:SQL Types:Ch 10"
        "raft/mod.rs:Raft Log:Ch 14-16"
        "raft/wal.rs:Write-Ahead Log:Ch 16"
        "lib.rs:Database Struct:Ch 17"
        "main.rs:REPL:Ch 17"
    )

    local active=0
    local total=${#detail_files[@]}
    local results=()

    for entry in "${detail_files[@]}"; do
        IFS=':' read -r file label chapter <<< "$entry"
        local full_path="$CODE_DIR/my-toydb/src/$file"

        if [[ -f "$full_path" ]]; then
            # Check if file still has only TODO/placeholder content
            # Count non-comment, non-blank lines (indicates real code)
            local lines=0
            lines=$(sed '/^\s*\/\//d; /^\s*$/d' "$full_path" | wc -l | tr -d ' ')
            local has_todo=0
            has_todo=$(grep -c 'TODO' "$full_path" || true)

            if [[ "$lines" -gt 2 && "$has_todo" -eq 0 ]]; then
                active=$((active + 1))
                results+=("${GREEN}  ✓ ${label} ${DIM}(${chapter})${RESET}")
                update_progress "cumulative" "$file" "active"
            else
                results+=("${DIM}  · ${label} (${chapter})${RESET}")
            fi
        fi
    done

    echo -e "\n${BOLD}Cumulative Project${RESET}  ${active}/${total} layers"
    draw_bar "$active" "$total"
    for r in "${results[@]}"; do
        echo -e "$r"
    done
}

# ─── Check capstone challenges ────────────────────────────────────
check_capstone() {
    local passed=0
    local total=8
    local results=()

    for i in $(seq 1 8); do
        # Find the exercise file (not solution)
        local ex_file
        ex_file=$(ls "$CODE_DIR"/capstone/src/bin/c${i}_*.rs 2>/dev/null | grep -v solution | head -1 || true)

        if [[ -z "$ex_file" ]]; then
            results+=("${DIM}  · C${i}${RESET}")
            continue
        fi

        local label
        label=$(basename "$ex_file" .rs | sed "s/^c${i}_//" | tr '_' ' ')

        if $QUICK; then
            if ! grep -q 'todo!()' "$ex_file" 2>/dev/null; then
                passed=$((passed + 1))
                results+=("${GREEN}  ✓ C${i}: ${label}${RESET}")
                update_progress "capstone" "c${i}" "pass"
            else
                results+=("${DIM}  · C${i}: ${label}${RESET}")
            fi
        else
            local bin_name
            bin_name=$(basename "$ex_file" .rs | sed 's/_/-/g')
            if (cd "$CODE_DIR/capstone" && cargo test --bin "$bin_name" 2>&1 | grep -q "test result: ok"); then
                passed=$((passed + 1))
                results+=("${GREEN}  ✓ C${i}: ${label}${RESET}")
                update_progress "capstone" "c${i}" "pass"
            else
                results+=("${DIM}  · C${i}: ${label}${RESET}")
            fi
        fi
    done

    echo -e "\n${BOLD}Capstone Challenges${RESET}  ${passed}/${total}"
    draw_bar "$passed" "$total"
    for r in "${results[@]}"; do
        echo -e "$r"
    done
}

# ─── Streak & Timeline ────────────────────────────────────────────
show_streak() {
    python3 -c "
import json
from datetime import date, timedelta

with open('$PROGRESS_FILE') as f:
    data = json.load(f)

log = sorted(set(data.get('activity_log', [])))
today = date.today()

# ── Streak ──
streak = 0
if log:
    dates_set = set(log)
    check = today
    if today.isoformat() not in dates_set:
        check = today - timedelta(days=1)
        if check.isoformat() not in dates_set:
            check = None
    if check:
        while check.isoformat() in dates_set:
            streak += 1
            check -= timedelta(days=1)

# Fire emoji
fire = ''
if streak >= 7: fire = '🔥🔥🔥'
elif streak >= 3: fire = '🔥🔥'
elif streak > 0: fire = '🔥'

# ── Timeline ──
start_str = data.get('start_date', today.isoformat())
target_str = data.get('target_date', (today + timedelta(weeks=12)).isoformat())
start = date.fromisoformat(start_str)
target = date.fromisoformat(target_str)
total_plan_days = (target - start).days or 1
elapsed = (today - start).days
remaining = max(0, (target - today).days)
weeks_elapsed = elapsed / 7
weeks_total = total_plan_days / 7
pct_time = min(100, elapsed * 100 // total_plan_days)

# Progress vs time
ex = sum(1 for v in data.get('exercises', {}).values() if v == 'pass')
cu = sum(1 for v in data.get('cumulative', {}).values() if v == 'active')
ca = sum(1 for v in data.get('capstone', {}).values() if v == 'pass')
total_done = ex + cu + ca
total_all = 38
pct_done = total_done * 100 // total_all if total_all > 0 else 0

if pct_done >= pct_time:
    pace = '✅ On track'
elif pct_done >= pct_time - 15:
    pace = '⚠️  Slightly behind'
else:
    pace = '🏃 Behind — consider extra time this week'

# ── 7-day activity ──
days_display = ''
labels_display = ''
for i in range(6, -1, -1):
    d = today - timedelta(days=i)
    days_display += ('█ ' if d.isoformat() in set(log) else '░ ')
    labels_display += (d.strftime('%a')[0] + ' ')

# ── Study plan weeks ──
STUDY_PLAN = [
    ('Ch 1-2',  'KV store + storage trait'),
    ('Ch 3-4',  'BitCask + serialization'),
    ('Ch 5',    'MVCC transactions'),
    ('Ch 6-7',  'SQL lexer + parser'),
    ('Ch 8-9',  'Query planner + optimizer'),
    ('Ch 10-11','Executor + joins'),
    ('Ch 12-13','Client-server + async'),
    ('Ch 14',   'Raft leader election'),
    ('Ch 15-16','Raft log + WAL'),
    ('Ch 17-18','Integration + testing'),
    ('C1-C4',   'Capstone challenges'),
    ('C5-C8',   'Capstone + review'),
]

current_week = min(int(weeks_elapsed) + 1, 12)

# ── Print ──
print()
print('━' * 48)
print()

# Timeline
print(f'  Started: {start_str}  →  Target: {target_str}')
print(f'  Day {elapsed} of {total_plan_days}  |  Week {current_week} of {int(weeks_total)}  |  {remaining} days left')

# Time bar
tw = 30
tf = min(tw, elapsed * tw // total_plan_days)
te = tw - tf
tbar = '▓' * tf + '░' * te
print(f'  {tbar}  {pct_time}% time elapsed')

# Pace
print(f'  Progress: {pct_done}% done vs {pct_time}% time  →  {pace}')
print()

# This week / Next week
chs, topic = STUDY_PLAN[current_week - 1]
print(f'  📅 This week (W{current_week}): {chs} — {topic}')
if current_week < 12:
    nchs, ntopic = STUDY_PLAN[current_week]
    print(f'     Next week (W{current_week+1}): {nchs} — {ntopic}')
print()

# Streak
if streak > 0:
    print(f'  {fire} Streak: {streak} day(s)  |  {len(log)} total active days')
else:
    print(f'  No active streak — commit some code today to start one!')

print(f'  Last 7 days: {days_display.strip()}')
print(f'               {labels_display.strip()}')
print()
print('━' * 48)
" 2>/dev/null
}

# ─── Progress bar drawing ─────────────────────────────────────────
draw_bar() {
    local done=$1
    local total=$2
    local width=30

    if [[ "$total" -eq 0 ]]; then
        return
    fi

    local filled=$((done * width / total))
    local empty=$((width - filled))
    local pct=$((done * 100 / total))

    local bar=""
    for ((i = 0; i < filled; i++)); do bar+="█"; done
    for ((i = 0; i < empty; i++)); do bar+="░"; done

    local color="$RED"
    if [[ "$pct" -ge 80 ]]; then
        color="$GREEN"
    elif [[ "$pct" -ge 40 ]]; then
        color="$YELLOW"
    fi

    echo -e "  ${color}${bar}${RESET}  ${pct}%"
}

# ─── Overall progress ────────────────────────────────────────────
show_overall() {
    # Calculate totals from progress file
    local result
    result=$(python3 -c "
import json
with open('$PROGRESS_FILE') as f:
    data = json.load(f)

ex = sum(1 for v in data.get('exercises', {}).values() if v == 'pass')
cu = sum(1 for v in data.get('cumulative', {}).values() if v == 'active')
ca = sum(1 for v in data.get('capstone', {}).values() if v == 'pass')

total_done = ex + cu + ca
total_all = 18 + 12 + 8  # exercises + layers + capstone
print(f'{total_done}|{total_all}')
" 2>/dev/null || echo "0|38")

    IFS='|' read -r done total <<< "$result"
    local pct=$((done * 100 / total))

    echo -e "\n${BOLD}${CYAN}  toydb-book ${RESET}${BOLD}— Learn Rust by Building a Database${RESET}"
    echo -e "  ${DIM}Overall: ${done}/${total} milestones (${pct}%)${RESET}"
    draw_bar "$done" "$total"
}

# ─── Generate PROGRESS.md ─────────────────────────────────────────
generate_progress_md() {
    local md_file="$BOOK_DIR/PROGRESS.md"

    python3 -c "
import json
from datetime import date, timedelta

with open('$PROGRESS_FILE') as f:
    data = json.load(f)

exercises = data.get('exercises', {})
cumulative = data.get('cumulative', {})
capstone = data.get('capstone', {})
log = sorted(set(data.get('activity_log', [])))

ex_pass = sum(1 for v in exercises.values() if v == 'pass')
cu_active = sum(1 for v in cumulative.values() if v == 'active')
ca_pass = sum(1 for v in capstone.values() if v == 'pass')
total_done = ex_pass + cu_active + ca_pass
total_all = 18 + 12 + 8
pct = total_done * 100 // total_all if total_all > 0 else 0

# Streak
today = date.today()
streak = 0
if log:
    dates_set = set(log)
    check = today
    if today.isoformat() not in dates_set:
        check = today - timedelta(days=1)
    while check.isoformat() in dates_set:
        streak += 1
        check -= timedelta(days=1)

# Timeline
start_str = data.get('start_date', today.isoformat())
target_str = data.get('target_date', (today + timedelta(weeks=12)).isoformat())
start = date.fromisoformat(start_str)
target = date.fromisoformat(target_str)
total_plan_days = (target - start).days or 1
elapsed = (today - start).days
remaining = max(0, (target - today).days)
weeks_elapsed = elapsed / 7
weeks_total = total_plan_days / 7
pct_time = min(100, elapsed * 100 // total_plan_days)

if pct >= pct_time:
    pace = '✅ On track'
elif pct >= pct_time - 15:
    pace = '⚠️  Slightly behind'
else:
    pace = '🏃 Behind — consider extra time this week'

# Progress bar helper
def bar(done, total, width=20):
    if total == 0:
        return '░' * width
    filled = done * width // total
    return '█' * filled + '░' * (width - filled)

def time_bar(elapsed, total, width=20):
    filled = min(width, elapsed * width // total) if total > 0 else 0
    return '▓' * filled + '░' * (width - filled)

# 7-day activity
days_display = ''
labels_display = ''
for i in range(6, -1, -1):
    d = today - timedelta(days=i)
    days_display += ('█ ' if d.isoformat() in set(log) else '░ ')
    labels_display += (d.strftime('%a')[0] + ' ')

# Chapter details
ch_lines = []
for i in range(1, 19):
    key = f'ch{i:02d}'
    if exercises.get(key) == 'pass':
        ch_lines.append(f'| Ch {i:02d} | ✅ Complete |')
    else:
        ch_lines.append(f'| Ch {i:02d} | ⬜ Not started |')

# Cumulative details
cu_files = [
    ('storage/mod.rs', 'Storage Trait', 'Ch 2'),
    ('storage/memory.rs', 'BTreeMap Engine', 'Ch 1-2'),
    ('error.rs', 'Error Types', 'Ch 3'),
    ('sql/lexer.rs', 'SQL Lexer', 'Ch 6'),
    ('sql/parser.rs', 'SQL Parser', 'Ch 7'),
    ('sql/planner.rs', 'Query Planner', 'Ch 8-9'),
    ('sql/executor.rs', 'Query Executor', 'Ch 10-11'),
    ('sql/types.rs', 'SQL Types', 'Ch 10'),
    ('raft/mod.rs', 'Raft Log', 'Ch 14-16'),
    ('raft/wal.rs', 'Write-Ahead Log', 'Ch 16'),
    ('lib.rs', 'Database Struct', 'Ch 17'),
    ('main.rs', 'REPL', 'Ch 17'),
]
cu_lines = []
for f, label, ch in cu_files:
    if cumulative.get(f) == 'active':
        cu_lines.append(f'| {label} | {ch} | ✅ |')
    else:
        cu_lines.append(f'| {label} | {ch} | ⬜ |')

# Capstone details
ca_names = [
    'KV Range Query', 'SQL Expression Eval', 'Query Plan Builder',
    'Transaction Scheduler', 'Raft Log Compaction', 'Index Scan Optimizer',
    'Deadlock Detector', 'Distributed Counter'
]
ca_lines = []
for i, name in enumerate(ca_names, 1):
    if capstone.get(f'c{i}') == 'pass':
        ca_lines.append(f'| C{i} | {name} | ✅ |')
    else:
        ca_lines.append(f'| C{i} | {name} | ⬜ |')

# Fire emoji
fire = '🔥' * min(streak // 3 + 1, 3) if streak > 0 else ''

# Study plan weeks
STUDY_PLAN = [
    ('Ch 1-2',  'KV store + storage trait'),
    ('Ch 3-4',  'BitCask + serialization'),
    ('Ch 5',    'MVCC transactions'),
    ('Ch 6-7',  'SQL lexer + parser'),
    ('Ch 8-9',  'Query planner + optimizer'),
    ('Ch 10-11','Executor + joins'),
    ('Ch 12-13','Client-server + async'),
    ('Ch 14',   'Raft leader election'),
    ('Ch 15-16','Raft log + WAL'),
    ('Ch 17-18','Integration + testing'),
    ('C1-C4',   'Capstone challenges'),
    ('C5-C8',   'Capstone + review'),
]

current_week = min(int(weeks_elapsed) + 1, 12)

# Study plan table
plan_lines = []
for i, (chs, topic) in enumerate(STUDY_PLAN, 1):
    if i < current_week:
        marker = '✅'
    elif i == current_week:
        marker = '👉'
    else:
        marker = ''
    plan_lines.append(f'| {marker} {i} | {chs} | {topic} |')

chs_now, topic_now = STUDY_PLAN[current_week - 1]

md = f'''# My toydb-book Progress

> Auto-generated by \`track.sh\` — last updated {today.isoformat()}

## Timeline

| | |
|---|---|
| **Started** | {start_str} |
| **Target** | {target_str} ({int(weeks_total)} weeks) |
| **Day** | {elapsed} of {total_plan_days} |
| **Week** | {current_week} of {int(weeks_total)} |
| **Remaining** | {remaining} days |
| **This week** | W{current_week}: {chs_now} — {topic_now} |

\`\`\`
Time:     {time_bar(elapsed, total_plan_days, 30)}  {pct_time}%
Progress: {bar(total_done, total_all, 30)}  {pct}%
\`\`\`

**Pace: {pace}**

## 12-Week Study Plan

| Week | Chapters | Focus |
|:----:|:---------|:------|
{chr(10).join(plan_lines)}

## Overall

\`\`\`
{bar(total_done, total_all, 30)}  {pct}%  ({total_done}/{total_all} milestones)
\`\`\`

{fire + ' ' if fire else ''}**Streak: {streak} day(s)**  |  {len(log)} total active days{(' | Since ' + log[0]) if log else ''}

\`\`\`
Last 7 days: {days_display.strip()}
             {labels_display.strip()}
\`\`\`

---

## Chapter Exercises ({ex_pass}/18)

\`\`\`
{bar(ex_pass, 18)}  {ex_pass}/18
\`\`\`

| Chapter | Status |
|---------|--------|
{chr(10).join(ch_lines)}

## Cumulative Project — my-toydb ({cu_active}/12 layers)

\`\`\`
{bar(cu_active, 12)}  {cu_active}/12
\`\`\`

| Layer | Chapter | Status |
|-------|---------|--------|
{chr(10).join(cu_lines)}

## Capstone Challenges ({ca_pass}/8)

\`\`\`
{bar(ca_pass, 8)}  {ca_pass}/8
\`\`\`

| # | Challenge | Status |
|---|-----------|--------|
{chr(10).join(ca_lines)}
'''

with open('$md_file', 'w') as f:
    f.write(md)
" 2>/dev/null

    echo -e "\n${DIM}  Updated PROGRESS.md${RESET}"
}

# ─── Main ─────────────────────────────────────────────────────────
main() {
    init_progress

    if $QUICK; then
        echo -e "${DIM}  (quick mode — checking file changes only, use ./track.sh for full test run)${RESET}"
    fi

    check_exercises
    check_cumulative
    check_capstone
    show_overall
    show_streak
    generate_progress_md
}

main
