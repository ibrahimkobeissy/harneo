#!/usr/bin/env bash
# board.sh: move a repo's issues between Status columns of a GitHub Project (v2).
# Reads OWNER, REPO and PROJECT from .claude/ticket-flow.conf at the repo root.
# Needs gh with the `project` scope (gh auth refresh -s project).
set -euo pipefail

usage() {
  cat <<'EOF'
usage: board.sh <command>
  init <project#> [owner] set up this repo: config, label, .gitignore, board options
  check                 verify config, gh auth and the board's Status options
  setup                 create the missing Status options (keeps every card's status)
  list [status]         list the repo's board items, optionally one status only
  add <issue#> [status] add an issue to the board (default status: backlog)
  set <issue#>... <status> set the status of one or more issues (adds them to the board if missing)
  sync                  apply status:* labels (left by cloud sessions) to the board, then remove them
statuses: backlog | ready | progress | review | done
EOF
  exit 2
}

die() { echo "board.sh: $*" >&2; exit 1; }

root=$(git rev-parse --show-toplevel)
conf="$root/.claude/ticket-flow.conf"
conf_get() { sed -n "s/^$1=//p" "$conf" | head -n1 | tr -d '[:space:]'; }

load_conf() {
  [ -f "$conf" ] || die "missing $conf (run: board.sh init <project#>)"
  OWNER=$(conf_get OWNER)
  REPO=$(conf_get REPO)
  PROJECT=$(conf_get PROJECT)
  [ -n "$OWNER" ] && [ -n "$REPO" ] && [ -n "$PROJECT" ] || die "$conf must set OWNER, REPO and PROJECT"
}

# Everything ticket-flow needs in a repo. Safe to rerun: existing pieces are kept.
init_repo() {
  local project=$1 owner=${2:-} repo
  command -v jq >/dev/null || die "jq is required"
  if [ -f "$conf" ]; then
    echo "config: keeping $conf"
  else
    repo=$(gh repo view --json nameWithOwner --jq .nameWithOwner) || die "cannot read this repo from GitHub"
    [ -n "$owner" ] || owner=${repo%%/*}
    mkdir -p "$root/.claude"
    printf 'OWNER=%s\nREPO=%s\nPROJECT=%s\n' "$owner" "$repo" "$project" >"$conf"
    echo "config: wrote $conf"
  fi
  load_conf
  gh label create needs-decision -R "$REPO" --color D93F0B --force \
    --description "Waiting on a decision from the user" >/dev/null
  echo "label: needs-decision"
  if grep -qxF '.claude/worktrees/' "$root/.gitignore" 2>/dev/null; then
    echo ".gitignore: already ignores .claude/worktrees/"
  else
    [ ! -s "$root/.gitignore" ] || [ -z "$(tail -c1 "$root/.gitignore")" ] || echo >>"$root/.gitignore"
    echo '.claude/worktrees/' >>"$root/.gitignore"
    echo ".gitignore: added .claude/worktrees/"
  fi
  setup_options
  echo "left to do by hand:"
  echo "  - add to CLAUDE.md: \"Tasks: follow the ticket-flow skill.\""
  echo "  - optional: project settings > Workflows > Auto-add to project (new items in Backlog)"
}

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

# Board data is fetched once per run: GitHub rate-limits bursts of Projects API calls.
PROJECT_ID="" FIELD_JSON="" ITEMS_JSON=""
fetch_field() {
  gh project field-list "$PROJECT" --owner "$OWNER" --format json \
    --jq '.fields[] | select(.name == "Status")'
}
load_board() {
  [ -n "$PROJECT_ID" ] || PROJECT_ID=$(gh project view "$PROJECT" --owner "$OWNER" --format json --jq .id)
  [ -n "$FIELD_JSON" ] || FIELD_JSON=$(fetch_field)
  [ -n "$ITEMS_JSON" ] || ITEMS_JSON=$(gh project item-list "$PROJECT" --owner "$OWNER" --limit 1000 --format json)
}
project_id() { echo "$PROJECT_ID"; }
status_field() { echo "$FIELD_JSON"; }

# Option id for a board option name, matched case-insensitively.
option_id() {
  local want
  want=$(echo "$1" | tr '[:upper:]' '[:lower:]')
  status_field | jq -r --arg w "$want" '.options[] | select((.name | ascii_downcase) == $w) | .id'
}

item_id() {
  echo "$ITEMS_JSON" | jq -r --arg repo "$REPO" --argjson n "$1" \
    '.items[] | select(.content.repository == $repo and .content.number == $n) | .id'
}

graphql() { gh api graphql --input -; } # request body on stdin

# Status options with color and description (field-list only gives id and name).
field_options() {
  jq -n --arg id "$1" '{
      query: "query($id: ID!) { node(id: $id) { ... on ProjectV2SingleSelectField { options { id name color description } } } }",
      variables: {id: $id}}' \
    | graphql | jq '.data.node.options'
}

WANTED='[
  {"name": "Backlog", "color": "GRAY", "description": "Blocked, waiting on a decision, or not triaged"},
  {"name": "Ready", "color": "BLUE", "description": "Not blocked, no decision pending: can be picked"},
  {"name": "In progress", "color": "YELLOW", "description": "A subagent or the orchestrator is working on it"},
  {"name": "In review", "color": "PURPLE", "description": "Finished, waiting for the orchestrator to validate and merge"},
  {"name": "Done", "color": "GREEN", "description": "Merged to main and pushed; issue closed"}
]'

# GitHub can only replace the whole option list, and replaced options get new ids,
# which can clear cards' statuses. So snapshot every card's status, replace the
# list (the five wanted options, plus any other option still in use), then
# re-apply each card's status by name.
setup_options() {
  local field_id current items used new dropped kept pid item name opt
  load_board
  field_id=$(status_field | jq -r .id)
  [ -n "$field_id" ] || die "board has no Status field"
  current=$(field_options "$field_id")
  items=$(echo "$ITEMS_JSON" | jq '[.items[] | select(.status != null) | {id, status}]')
  used=$(echo "$items" | jq '[.[].status] | unique')
  new=$(jq -n --argjson cur "$current" --argjson want "$WANTED" --argjson used "$used" '
    ($want | map(.name | ascii_downcase)) as $wn
    | [$want[] as $w
        | ($cur | map(select((.name | ascii_downcase) == ($w.name | ascii_downcase))))[0] as $c
        | {name: ($c.name // $w.name), color: ($c.color // $w.color),
           description: (if ($c.description // "") == "" then $w.description else $c.description end)}]
    + [$cur[] | select(.name | ascii_downcase | IN($wn[]) | not) | select(.name | IN($used[]))
        | {name, color, description}]')
  missing=$(jq -rn --argjson cur "$current" --argjson want "$WANTED" \
    '$want[].name | select(ascii_downcase | IN($cur[].name | ascii_downcase) | not)')
  dropped=$(jq -rn --argjson cur "$current" --argjson new "$new" '$cur[].name | select(IN($new[].name) | not)')
  kept=$(jq -rn --argjson cur "$current" --argjson want "$WANTED" --argjson used "$used" \
    '$cur[].name | select(ascii_downcase | IN($want[].name | ascii_downcase) | not) | select(IN($used[]))')
  if [ -z "$missing" ] && [ -z "$dropped" ]; then
    echo "Status options OK, nothing to do"
  else
    [ -n "$missing" ] && echo "$missing" | sed 's/^/adding: /'
    [ -n "$dropped" ] && echo "$dropped" | sed 's/^/removing unused: /'
    jq -n --arg f "$field_id" --argjson o "$new" '{
        query: "mutation($f: ID!, $o: [ProjectV2SingleSelectFieldOptionInput!]) { updateProjectV2Field(input: {fieldId: $f, singleSelectOptions: $o}) { clientMutationId } }",
        variables: {f: $f, o: $o}}' | graphql >/dev/null
    FIELD_JSON=$(fetch_field) # option ids changed
    pid=$(project_id)
    echo "$items" | jq -r '.[] | [.id, .status] | @tsv' | while IFS=$'\t' read -r item name; do
      opt=$(option_id "$name")
      gh project item-edit --id "$item" --project-id "$pid" \
        --field-id "$field_id" --single-select-option-id "$opt" >/dev/null
    done
    echo "restored the status of $(echo "$items" | jq length) card(s)"
  fi
  [ -z "$kept" ] || echo "$kept" | sed 's/^/kept (still used by cards, move them and rerun setup to remove): /'
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
[ "$cmd" = init ] || load_conf
case "$cmd" in
  init)
    [ -n "${2:-}" ] || usage
    init_repo "$2" "${3:-}"
    ;;
  check)
    command -v jq >/dev/null || die "jq is required"
    gh project view "$PROJECT" --owner "$OWNER" --format json --jq '"board: " + .title' \
      || die "cannot read project $PROJECT of $OWNER (gh auth refresh -s project?)"
    load_board
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
    [ "$missing" -eq 0 ] && echo "Status options OK" || die "run: board.sh setup"
    ;;
  setup)
    command -v jq >/dev/null || die "jq is required"
    setup_options
    ;;
  list)
    filter=""
    [ -n "${2:-}" ] && filter=$(status_name "$2")
    load_board
    echo "$ITEMS_JSON" \
      | jq -r --arg repo "$REPO" --arg f "$filter" '
          .items[]
          | select(.content.repository == $repo)
          | select($f == "" or ((.status // "") | ascii_downcase) == ($f | ascii_downcase))
          | "#\(.content.number)\t\(.status // "(none)")\t\(.content.title)"'
    ;;
  add)
    [ -n "${2:-}" ] || usage
    load_board
    set_status "$2" "${3:-backlog}"
    ;;
  set)
    # set <issue#>... <status>: several issues in one run share the fetched board data
    [ $# -ge 3 ] || usage
    status=${!#}
    status_name "$status" >/dev/null
    load_board
    for n in "${@:2:$(($# - 2))}"; do set_status "$n" "$status"; done
    ;;
  sync)
    load_board
    for s in $STATUSES; do
      for n in $(gh issue list -R "$REPO" --state all --label "status:$s" --json number --jq '.[].number'); do
        set_status "$n" "$s"
        gh issue edit "$n" -R "$REPO" --remove-label "status:$s" >/dev/null
      done
    done
    ;;
  *) usage ;;
esac
