#!/usr/bin/env bash
# setup: check, and after confirmation install, what building Harneo from source needs
# (Linux and macOS; Windows has scripts/setup.ps1). Rust through rustup, Node at the
# required major, and the OS webview/build packages Tauri 2 needs.
#   setup.sh            install what is missing, after a confirmation
#   setup.sh --dev      also the contributor gate tools (gitleaks, osv-scanner, semgrep,
#                       diff-cover, cargo-llvm-cov, jq)
#   setup.sh --check    install nothing; report what is missing, exit 1 if anything is
#   setup.sh --yes      skip the confirmation (HARNEO_SETUP_YES=1 does the same)
# Test overrides: HARNEO_SETUP_OS=linux|macos, HARNEO_SETUP_PM=pacman|apt|dnf|brew.
set -euo pipefail
cd "$(dirname "$0")/.."

dev=0 check=0 yes=${HARNEO_SETUP_YES:-0}
for arg in "$@"; do
  case "$arg" in
    --dev) dev=1 ;;
    --check) check=1 ;;
    --yes | -y) yes=1 ;;
    -h | --help)
      sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'
      exit 0
      ;;
    *)
      echo "setup: unknown option '$arg' (use --dev, --check, --yes)" >&2
      exit 2
      ;;
  esac
done

have() { command -v "$1" >/dev/null 2>&1; }

# --- OS and package manager -------------------------------------------------------------
os=${HARNEO_SETUP_OS:-}
if [[ -z "$os" ]]; then
  case "$(uname -s)" in
    Darwin) os=macos ;;
    Linux) os=linux ;;
    *)
      echo "setup: unsupported OS '$(uname -s)'. On Windows run scripts/setup.ps1." >&2
      exit 2
      ;;
  esac
fi
pm=${HARNEO_SETUP_PM:-}
if [[ -z "$pm" ]]; then
  if [[ "$os" == macos ]]; then
    pm=brew
  elif have pacman; then
    pm=pacman
  elif have apt-get; then
    pm=apt
  elif have dnf; then
    pm=dnf
  fi
fi
if [[ -z "$pm" ]] || { [[ "$os" == macos ]] && [[ "$pm" != brew ]]; }; then
  echo "setup: no supported package manager found (need pacman, apt, dnf or brew)." >&2
  exit 2
fi
if [[ "$os" == macos ]] && ! have brew && ((check == 0)); then
  echo "setup: Homebrew is required on macOS. Install it from https://brew.sh and rerun." >&2
  exit 2
fi
echo "setup: $os, package manager: $pm"

# Tauri 2 prerequisites per package manager.
case "$pm" in
  pacman) pkgs=(webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool) ;;
  apt) pkgs=(libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev) ;;
  dnf) pkgs=(webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel) ;;
  brew) pkgs=() ;;
esac
dnf_group="C Development Tools and Libraries"

pkg_installed() {
  case "$pm" in
    pacman) pacman -Q "$1" >/dev/null 2>&1 ;;
    apt) dpkg -s "$1" >/dev/null 2>&1 ;;
    dnf) rpm -q "$1" >/dev/null 2>&1 ;;
    brew) brew list "$1" >/dev/null 2>&1 ;;
  esac
}

missing=() # human-readable lines for the report
plan=()    # "priv|command" or "user|command", run after confirmation

add_missing() { missing+=("$1"); }
add_plan() { plan+=("$1|$2"); }

# --- build prerequisites ----------------------------------------------------------------
min_node=22
if [[ -f package.json ]]; then
  pj=$(<package.json)
  if [[ "$pj" =~ \"node\"[[:space:]]*:[[:space:]]*\"[^0-9\"]*([0-9]+) ]]; then
    min_node=${BASH_REMATCH[1]}
  fi
fi

# Rust, through rustup (rust-toolchain.toml pins the toolchain).
if ! have rustup; then
  add_missing "rustup (Rust toolchain manager)"
  case "$pm" in
    pacman) add_plan priv "sudo pacman -S --needed --noconfirm rustup && rustup default stable" ;;
    brew) add_plan user "brew install rustup && rustup default stable" ;;
    *) add_plan user "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y" ;;
  esac
fi

# Node at the minimum major.
node_ok=0
if have node; then
  node_v=$(node --version 2>/dev/null || true)
  if [[ "$node_v" =~ ^v([0-9]+)\. ]] && ((BASH_REMATCH[1] >= min_node)); then
    node_ok=1
  else
    add_missing "node >= $min_node (found ${node_v:-unknown})"
  fi
else
  add_missing "node >= $min_node"
fi
if ((node_ok == 0)); then
  if have mise; then
    add_plan user "mise use -g node@$min_node"
  else
    case "$pm" in
      pacman) add_plan priv "sudo pacman -S --needed --noconfirm nodejs npm" ;;
      apt) add_plan priv "sudo apt-get install -y nodejs npm" ;;
      dnf) add_plan priv "sudo dnf install -y nodejs npm" ;;
      brew) add_plan user "brew install node@$min_node" ;;
    esac
  fi
fi

# OS packages.
if [[ "$os" == macos ]]; then
  if ! xcode-select -p >/dev/null 2>&1; then
    add_missing "Xcode command line tools"
    add_plan user "xcode-select --install"
  fi
else
  miss=()
  for p in "${pkgs[@]}"; do
    pkg_installed "$p" || miss+=("$p")
  done
  if ((${#miss[@]})); then
    add_missing "${pm} packages: ${miss[*]}"
    case "$pm" in
      pacman) add_plan priv "sudo pacman -S --needed --noconfirm ${miss[*]}" ;;
      apt) add_plan priv "sudo apt-get install -y ${miss[*]}" ;;
      dnf) add_plan priv "sudo dnf install -y ${miss[*]}" ;;
    esac
  fi
  if [[ "$pm" == dnf ]]; then
    groups=$(dnf group list --installed 2>/dev/null || true)
    if [[ "$groups" != *"$dnf_group"* ]]; then
      add_missing "dnf group: $dnf_group"
      add_plan priv "sudo dnf group install -y \"$dnf_group\""
    fi
  fi
fi

# --- contributor gate tools (--dev) -------------------------------------------------------
if ((dev)); then
  need_pipx=0
  dev_tool() { # dev_tool <command> <mise spec> <pipx|cargo|pm|manual> <fallback>
    local cmd=$1 spec=$2 kind=$3 fallback=$4
    have "$cmd" && return 0
    add_missing "$cmd (contributor gate tool)"
    if have mise; then
      add_plan user "mise use -g $spec"
      return 0
    fi
    case "$kind" in
      pipx)
        need_pipx=1
        add_plan user "pipx install $fallback"
        ;;
      cargo) add_plan user "cargo install --locked $fallback" ;;
      pm)
        case "$pm" in
          pacman) add_plan priv "sudo pacman -S --needed --noconfirm $fallback" ;;
          apt) add_plan priv "sudo apt-get install -y $fallback" ;;
          dnf) add_plan priv "sudo dnf install -y $fallback" ;;
          brew) add_plan user "brew install $fallback" ;;
        esac
        ;;
      manual) add_plan manual "install $cmd manually: $fallback" ;;
    esac
  }
  dev_tool jq jq pm jq
  # gitleaks and osv-scanner: in pacman and brew; elsewhere a release download.
  for t in gitleaks osv-scanner; do
    if [[ "$pm" == pacman || "$pm" == brew ]]; then
      dev_tool "$t" "$t" pm "$t"
    else
      dev_tool "$t" "$t" manual "https://github.com/$([[ $t == gitleaks ]] && echo gitleaks/gitleaks || echo google/osv-scanner)/releases"
    fi
  done
  dev_tool semgrep pipx:semgrep pipx semgrep
  dev_tool diff-cover pipx:diff-cover pipx diff-cover
  dev_tool cargo-llvm-cov github:taiki-e/cargo-llvm-cov cargo cargo-llvm-cov
  if ((need_pipx)) && ! have pipx && ! have mise; then
    case "$pm" in
      pacman) add_plan priv "sudo pacman -S --needed --noconfirm python-pipx" ;;
      apt) add_plan priv "sudo apt-get install -y pipx" ;;
      dnf) add_plan priv "sudo dnf install -y pipx" ;;
      brew) add_plan user "brew install pipx" ;;
    esac
    # pipx must come first.
    last=${plan[${#plan[@]} - 1]}
    unset 'plan[${#plan[@]}-1]'
    plan=("$last" "${plan[@]}")
  fi
fi

# --- report -----------------------------------------------------------------------------
if ((${#missing[@]} == 0)); then
  echo "setup: everything is present."
  exit 0
fi
echo "setup: missing:"
for m in "${missing[@]}"; do echo "  - $m"; done

if ((check)); then
  hint="scripts/setup.sh"
  ((dev)) && hint+=" --dev"
  echo "setup: --check installs nothing. Run $hint to install." >&2
  exit 1
fi

echo "setup: will run:"
any_priv=0
for step in "${plan[@]}"; do
  kind=${step%%|*} cmd=${step#*|}
  case "$kind" in
    priv)
      any_priv=1
      echo "  [needs sudo] $cmd"
      ;;
    manual) echo "  [manual]     $cmd" ;;
    *) echo "  $cmd" ;;
  esac
done
((any_priv)) && echo "setup: the [needs sudo] steps run with administrator rights and may ask for your password."

if [[ "$yes" != 1 ]]; then
  if [[ ! -t 0 ]]; then
    echo "setup: not a terminal; rerun with --yes to confirm, or --check to only report." >&2
    exit 1
  fi
  read -r -p "setup: proceed? [y/N] " answer
  [[ "$answer" == [yY]* ]] || {
    echo "setup: cancelled, nothing changed."
    exit 1
  }
fi

failed=0
for step in "${plan[@]}"; do
  kind=${step%%|*} cmd=${step#*|}
  if [[ "$kind" == manual ]]; then
    echo "setup: TODO $cmd"
    failed=1
    continue
  fi
  [[ "$kind" == priv ]] && echo "setup: running with sudo: $cmd"
  bash -c "$cmd" || {
    echo "setup: FAILED: $cmd" >&2
    failed=1
  }
done
((failed == 0)) || {
  echo "setup: finished with problems; rerun scripts/setup.sh --check to see what is left." >&2
  exit 1
}
echo "setup: done. Rerun with --check to confirm."
