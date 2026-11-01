#!/usr/bin/env bash
# Self-test for scripts/setup.sh: fake pacman/dpkg/rpm/brew/... in a temp PATH, per OS.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

pass=0
failed=0
real_bash=$(command -v bash)

# Tauri 2 package lists, mirrored from the Tauri prerequisites docs.
arch_pkgs="webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libappindicator-gtk3 librsvg xdotool git"
deb_pkgs="libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev git"
fed_pkgs="webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel libxdo-devel git"

# stub <name> <script body>: executable fake in $bin.
stub() { printf '#!/bin/bash\n%s\n' "$2" >"$bin/$1" && chmod +x "$bin/$1"; }

# new_env <name> <installed packages...>: fresh stub dir; the package-manager stubs
# succeed only for the listed packages. Node 22 and rustup are present.
new_env() {
  bin="$tmp/$1/bin"
  kegs="$tmp/$1/kegs"
  mkdir -p "$bin"
  ln -s "$real_bash" "$bin/bash"
  ln -s "$(command -v dirname)" "$bin/dirname"
  shift
  installed=" $* "
  stub pacman '[[ "$1" == -Q ]] && [[ "'"$installed"'" == *" $2 "* ]]'
  stub dpkg '[[ "$1" == -s ]] && [[ "'"$installed"'" == *" $2 "* ]]'
  stub rpm '[[ "$1" == -q ]] && [[ "'"$installed"'" == *" $2 "* ]]'
  stub brew '[[ "$1" == --prefix ]] && { echo "'"$kegs"'/$2"; exit 0; }; [[ "$1" == list ]] && [[ "'"$installed"'" == *" $2 "* ]]'
  stub dnf '[[ "$1 $2" == "group list" ]] && echo "'"${dnf_groups:-C Development Tools and Libraries}"'"; exit 0'
  stub xcode-select '[[ "'"${xcode:-yes}"'" == yes ]]'
  stub node 'echo "'"${node_v:-v22.4.0}"'"'
  stub rustup 'exit 0'
  stub sudo 'echo "sudo $*" >>"'"$tmp"'/sudo.log"'
  : >"$tmp/sudo.log"
}

# run <os> <pm> [args...]: setup.sh with only the stubs on PATH, so nothing real leaks in.
run() {
  local os=$1 pmgr=$2
  shift 2
  PATH="$bin" HARNEO_SETUP_OS=$os HARNEO_SETUP_PM=$pmgr "$real_bash" "$here/setup.sh" "$@" >"$tmp/out" 2>&1 && rc=0 || rc=$?
}
expect_rc() { # expect_rc <label> <want>
  if [[ "$rc" == "$2" ]]; then pass=$((pass + 1)); else
    failed=$((failed + 1))
    echo "FAIL: $1 (wanted exit $2, got $rc)"
    sed 's/^/    /' "$tmp/out"
  fi
}
has_output() { # has_output <label> <text>
  if grep -Fq -- "$2" "$tmp/out"; then pass=$((pass + 1)); else
    failed=$((failed + 1))
    echo "FAIL: $1 (output lacks: $2)"
    sed 's/^/    /' "$tmp/out"
  fi
}
lacks_output() {
  if grep -Fq -- "$2" "$tmp/out"; then
    failed=$((failed + 1))
    echo "FAIL: $1 (output has: $2)"
  else pass=$((pass + 1)); fi
}

# --- all present: exit 0 -----------------------------------------------------------------
new_env arch $arch_pkgs
run linux pacman --check
expect_rc "arch all present" 0
has_output "arch all present message" "everything is present"

new_env deb $deb_pkgs
stub apt-get 'exit 0'
run linux apt --check
expect_rc "debian all present" 0

new_env fed $fed_pkgs
run linux dnf --check
expect_rc "fedora all present" 0

new_env mac
run macos brew --check
expect_rc "macos all present" 0

# --- one package missing: reported by name, exit 1 -----------------------------------------
new_env arch-miss ${arch_pkgs/xdotool/}
run linux pacman --check
expect_rc "arch missing package" 1
has_output "arch names the package" "xdotool"
lacks_output "arch does not blame others" "librsvg"

new_env deb-miss ${deb_pkgs/libxdo-dev/}
run linux apt --check
expect_rc "debian missing package" 1
has_output "debian names the package" "libxdo-dev"

new_env fed-miss ${fed_pkgs/libxdo-devel/}
run linux dnf --check
expect_rc "fedora missing package" 1
has_output "fedora names the package" "libxdo-devel"

dnf_groups="Other group"
new_env fed-group $fed_pkgs
run linux dnf --check
expect_rc "fedora missing group" 1
has_output "fedora names the group" "C Development Tools and Libraries"
unset dnf_groups

xcode=no
new_env mac-miss
run macos brew --check
expect_rc "macos missing xcode tools" 1
has_output "macos names xcode tools" "Xcode command line tools"
unset xcode

# --- git: a runtime prerequisite (git URL imports) ------------------------------------------
new_env arch-nogit ${arch_pkgs/ git/}
run linux pacman --check
expect_rc "arch missing git" 1
has_output "arch names git" "pacman packages: git"
new_env deb-nogit ${deb_pkgs/ git/}
run linux apt --check
expect_rc "debian missing git" 1
has_output "debian names git" "apt packages: git"
new_env fed-nogit ${fed_pkgs/ git/}
run linux dnf --check
expect_rc "fedora missing git" 1
has_output "fedora names git" "dnf packages: git"
xcode=no
new_env mac-nogit
run macos brew --check
expect_rc "macos without tools or git" 1
has_output "macos names git" "git (comes with the Xcode command line tools)"
stub git 'exit 0'
run macos brew --check
lacks_output "macos with git on PATH" "git (comes with"
unset xcode

# --- macOS: Homebrew's keg-only rustup and node@22 (#192) ----------------------------------
new_env mac-keg
rm "$bin/rustup" "$bin/node"
mkdir -p "$kegs/rustup/bin" "$kegs/node@22/bin"
printf '#!/bin/bash\nexit 0\n' >"$kegs/rustup/bin/rustup"
printf '#!/bin/bash\necho v22.4.0\n' >"$kegs/node@22/bin/node"
chmod +x "$kegs/rustup/bin/rustup" "$kegs/node@22/bin/node"
run macos brew --check
expect_rc "macos keg-only rustup and node count as present" 0
has_output "macos keg-only prints the PATH line" "$kegs/rustup/bin:$kegs/node@22/bin:\$HOME/.cargo/bin"

new_env mac-norust
rm "$bin/rustup"
run macos brew
has_output "macos rustup runs from its keg" 'brew install rustup && "$(brew --prefix rustup)/bin/rustup" default stable'
lacks_output "macos without kegs prints no PATH line" "keg-only"

# --- node and rustup -------------------------------------------------------------------------
node_v=v20.1.0
new_env node-old $arch_pkgs
run linux pacman --check
expect_rc "old node" 1
has_output "old node reported" "node >= 22 (found v20.1.0)"
unset node_v

new_env no-rustup $arch_pkgs
rm "$bin/rustup"
run linux pacman --check
expect_rc "missing rustup" 1
has_output "rustup reported" "rustup"

# --- --dev: gate tools -----------------------------------------------------------------------
new_env dev $arch_pkgs
run linux pacman --check --dev
expect_rc "dev tools missing" 1
has_output "dev names gitleaks" "gitleaks"
has_output "dev names cargo-llvm-cov" "cargo-llvm-cov"
new_env dev-no $arch_pkgs
run linux pacman --check
expect_rc "dev tools ignored without --dev" 0
for t in gitleaks osv-scanner semgrep diff-cover cargo-llvm-cov jq; do stub "$t" 'exit 0'; done
run linux pacman --check --dev
expect_rc "dev tools present" 0

# --- install: says sudo first, runs only what is missing --------------------------------------
new_env inst ${arch_pkgs/xdotool/}
run linux pacman --yes
expect_rc "install with --yes" 0
has_output "install announces sudo" "[needs sudo] sudo pacman -S --needed --noconfirm xdotool"
has_output "install runs with sudo notice" "running with sudo"
if [[ "$(<"$tmp/sudo.log")" == "sudo pacman -S --needed --noconfirm xdotool" ]]; then pass=$((pass + 1)); else
  failed=$((failed + 1))
  echo "FAIL: sudo received the wrong command: $(<"$tmp/sudo.log")"
fi
# Idempotent: nothing missing, nothing runs.
new_env inst2 $arch_pkgs
run linux pacman --yes
expect_rc "second run" 0
if [[ -z "$(<"$tmp/sudo.log")" ]]; then pass=$((pass + 1)); else
  failed=$((failed + 1))
  echo "FAIL: second run called sudo"
fi
# No terminal and no --yes: refuses without changing anything.
new_env noconf ${arch_pkgs/xdotool/}
run linux pacman </dev/null
expect_rc "no confirmation" 1
if [[ -z "$(<"$tmp/sudo.log")" ]]; then pass=$((pass + 1)); else
  failed=$((failed + 1))
  echo "FAIL: ran sudo without confirmation"
fi

echo "setup self-test: $pass passed, $failed failed"
((failed == 0))
