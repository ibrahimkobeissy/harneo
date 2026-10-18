#!/usr/bin/env bash
# board.sh: move a repo's issues between Status columns of a GitHub Project (v2).
# Reads OWNER, REPO and PROJECT from .claude/ticket-flow.conf at the repo root.
# Needs gh with the `project` scope (gh auth refresh -s project).
set -euo pipefail

usage() {
  cat <<'EOF'
usage: board.sh <command>
  check                 verify config, gh auth and the board's Status options
  list [status]         list the repo's board items, optionally one status only
  add <issue#> [status] add an issue to the board (default status: backlog)
  set <issue#> <status> set an issue's status (adds it to the board if missing)
  sync                  apply status:* labels (left by cloud sessions) to the board, then remove them
statuses: backlog | ready | progress | review | done
EOF
  exit 2
}

die() { echo "board.sh: $*" >&2; exit 1; }

root=$(git rev-parse --show-toplevel)
conf="$root/.claude/ticket-flow.conf"
[ -f "$conf" ] || die "missing $conf (needs OWNER=, REPO=, PROJECT=)"
conf_get() { sed -n "s/^$1=//p" "$conf" | head -n1 | tr -d '[:space:]'; }
OWNER=$(conf_get OWNER)
REPO=$(conf_get REPO)
PROJECT=$(conf_get PROJECT)
[ -n "$OWNER" ] && [ -n "$REPO" ] && [ -n "$PROJECT" ] || die "$conf must set OWNER, REPO and PROJECT"

STATUSES="backlog ready progress review done"

# Board option name for a short status key.
status_name() {
  case "$1" in
    backlog) echo "Backlog" ;;
    ready) echo "Ready" ;;
    progress) echo "In progress" ;;
    review) echo "In review" ;;
    done) echo "Done" ;;
    *) die "unknown status '$1' (use: $STATUSES)" ;;
  esac
}

project_id() {
  gh project view "$PROJECT" --owner "$OWNER" --format json --jq .id
}

status_field() {
  gh project field-list "$PROJECT" --owner "$OWNER" --format json \
    --jq '.fields[] | select(.name == "Status")'
}

# Option id for a board option name, matched case-insensitively.
option_id() {
  local want
  want=$(echo "$1" | tr '[:upper:]' '[:lower:]')
  status_field | jq -r --arg w "$want" '.options[] | select((.name | ascii_downcase) == $w) | .id'
}

item_id() {
  gh project item-list "$PROJECT" --owner "$OWNER" --limit 1000 --format json \
    --jq ".items[] | select(.content.repository == \"$REPO\" and .content.number == $1) | .id"
}

add_item() {
  gh project item-add "$PROJECT" --owner "$OWNER" \
    --url "https://github.com/$REPO/issues/$1" --format json --jq .id
}

set_status() {
  local n=$1 name item field_id opt
  name=$(status_name "$2")
  item=$(item_id "$n")
  [ -n "$item" ] || item=$(add_item "$n")
  field_id=$(status_field | jq -r .id)
  opt=$(option_id "$name")
  [ -n "$opt" ] || die "board has no Status option '$name' (run: board.sh check)"
  gh project item-edit --id "$item" --project-id "$(project_id)" \
    --field-id "$field_id" --single-select-option-id "$opt" >/dev/null
  echo "#$n → $name"
}

cmd=${1:-}
case "$cmd" in
  check)
    command -v jq >/dev/null || die "jq is required"
    gh project view "$PROJECT" --owner "$OWNER" --format json --jq '"board: " + .title' \
      || die "cannot read project $PROJECT of $OWNER (gh auth refresh -s project?)"
    field=$(status_field)
    [ -n "$field" ] || die "board has no Status field"
    missing=0
    for s in $STATUSES; do
      name=$(status_name "$s")
      if [ -z "$(option_id "$name")" ]; then
        echo "missing Status option: $name"
        missing=1
      fi
    done
    [ "$missing" -eq 0 ] && echo "Status options OK" || die "add the missing options in the project's settings"
    ;;
  list)
    filter=""
    [ -n "${2:-}" ] && filter=$(status_name "$2")
    gh project item-list "$PROJECT" --owner "$OWNER" --limit 1000 --format json \
      | jq -r --arg repo "$REPO" --arg f "$filter" '
          .items[]
          | select(.content.repository == $repo)
          | select($f == "" or ((.status // "") | ascii_downcase) == ($f | ascii_downcase))
          | "#\(.content.number)\t\(.status // "(none)")\t\(.content.title)"'
    ;;
  add)
    [ -n "${2:-}" ] || usage
    set_status "$2" "${3:-backlog}"
    ;;
  set)
    [ -n "${2:-}" ] && [ -n "${3:-}" ] || usage
    set_status "$2" "$3"
    ;;
  sync)
    for s in $STATUSES; do
      for n in $(gh issue list -R "$REPO" --state all --label "status:$s" --json number --jq '.[].number'); do
        set_status "$n" "$s"
        gh issue edit "$n" -R "$REPO" --remove-label "status:$s" >/dev/null
      done
    done
    ;;
  *) usage ;;
esac
