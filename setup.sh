#!/usr/bin/env bash
#
# toydb-book setup — run once after cloning
#
set -euo pipefail

BOOK_DIR="$(cd "$(dirname "$0")" && pwd)"
HOOK_DIR="$BOOK_DIR/.git/hooks"
HOOK_FILE="$HOOK_DIR/post-commit"

echo "Setting up toydb-book..."

# ─── Create personal branch ──────────────────────────────────────
CURRENT_BRANCH=$(git -C "$BOOK_DIR" branch --show-current)

if [[ "$CURRENT_BRANCH" == "main" || "$CURRENT_BRANCH" == "master" ]]; then
    echo ""
    read -rp "  Enter your name (for your personal branch, e.g. 'siva'): " USERNAME
    if [[ -n "$USERNAME" ]]; then
        BRANCH="progress/${USERNAME}"
        git -C "$BOOK_DIR" checkout -b "$BRANCH"
        echo "  Created branch: $BRANCH"
        echo "  Your work stays here — main branch stays clean for updates."
    else
        echo "  Skipped — staying on $CURRENT_BRANCH"
        echo "  (Tip: create a branch later with: git checkout -b progress/your-name)"
    fi
else
    echo "  Already on branch: $CURRENT_BRANCH"
fi

# ─── Install post-commit hook ────────────────────────────────────
if [[ -f "$HOOK_FILE" ]]; then
    echo "  Post-commit hook already exists — skipping"
else
    cat > "$HOOK_FILE" << 'EOF'
#!/usr/bin/env bash
#
# Post-commit hook: shows toydb-book progress and updates PROGRESS.md
#
BOOK_DIR="$(git rev-parse --show-toplevel)"
TRACK="$BOOK_DIR/track.sh"

if [[ -x "$TRACK" ]]; then
    echo ""
    "$TRACK" --quick
    echo ""

    # Auto-stage PROGRESS.md into this commit if it changed
    if [[ -f "$BOOK_DIR/PROGRESS.md" ]] && ! git diff --quiet "$BOOK_DIR/PROGRESS.md" 2>/dev/null; then
        git add "$BOOK_DIR/PROGRESS.md"
        git commit --amend --no-edit --quiet
    fi
fi
EOF
    chmod +x "$HOOK_FILE"
    echo "  Installed post-commit hook"
fi

# ─── Verify python3 ──────────────────────────────────────────────
if command -v python3 &>/dev/null; then
    echo "  python3 found"
else
    echo "  Warning: python3 not found — streak tracking won't work"
fi

echo ""
echo "Done! Your setup:"
echo "  Branch:   $(git -C "$BOOK_DIR" branch --show-current)"
echo "  Tracker:  ./track.sh (or ./track.sh --quick)"
echo "  Auto:     progress shows after every git commit"
echo ""
echo "To pull book updates later without losing your work:"
echo "  git fetch origin main"
echo "  git merge origin/main"
