name := 'cosmic-applet-ledger'
name-debug := 'cosmic-applet-ledger-debug'
appid := 'io.github.mrn3ff.cosmic-applet-ledger'
flatpak-manifest := 'packaging/' + appid + '.json'

rootdir := ''
prefix := home_directory() / '.local'

# Installation paths
base-dir := absolute_path(clean(rootdir / prefix))
cargo-target-dir := env('CARGO_TARGET_DIR', 'target')
appdata-dst := base-dir / 'share' / 'appdata' / appid + '.metainfo.xml'
bin-dst := base-dir / 'bin' / name
bin-debug-dst := base-dir / 'bin' / name-debug
desktop-dst := base-dir / 'share' / 'applications' / appid + '.desktop'
desktop-debug-dst := base-dir / 'share' / 'applications' / 'cosmic-applet-ledger-debug.desktop'
icon-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'scalable' / 'apps' / appid + '.svg'
icon-symbolic-dst := base-dir / 'share' / 'icons' / 'hicolor' / 'symbolic' / 'apps' / appid + '-symbolic.svg'

# Default recipe which runs `just build-release`
default: build-release

# Runs `cargo clean`
clean:
    cargo clean

# Clears the app cache at ~/.cache/cosmic-applet-ledger
clear-cache:
    rm -rf ~/.cache/cosmic-applet-ledger

# Clears Cursor session dirs (cookie_header, managed profile Cookies DB) under ~/.local/state/cosmic-applet-ledger/cursor-accounts

# Clears cookie/session dirs, snapshot cache, and full managed state (codex/claude/cursor/logs under ~/.local/state/cosmic-applet-ledger)
clear-all-data: clear-config clear-cache clear-accounts

# Clears COSMIC config at $XDG_CONFIG_HOME/cosmic/<appid> (default ~/.config/...)
clear-config:
    rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/cosmic/{{ appid }}"

# Clears managed account state at ~/.local/state/cosmic-applet-ledger
clear-accounts:
    rm -rf ~/.local/state/cosmic-applet-ledger

# Clears native logs at ~/.local/state/cosmic-applet-ledger/logs
clear-logs:
    rm -rf ~/.local/state/cosmic-applet-ledger/logs

# Removes vendored dependencies
clean-vendor:
    rm -rf .cargo vendor vendor.tar

# `cargo clean` and removes vendored dependencies
clean-dist: clean clean-vendor

# Compiles with debug profile
build-debug *args:
    cargo build {{args}}

# Compiles with release profile
build-release *args: (build-debug '--release' args)

# Compiles release profile with vendored dependencies
build-vendored *args: vendor-extract (build-release '--frozen --offline' args)

# Runs a clippy check
check *args:
    cargo clippy --all-features --all-targets {{args}} -- -D warnings

# Runs a clippy check with JSON message format
check-json: (check '--message-format=json')

# Run the application for testing purposes
run *args:
    env RUST_BACKTRACE=full cargo run --release {{args}}

# Run the debug build with synthetic demo data
run-demo *args:
    env LEDGER_DEMO=1 RUST_BACKTRACE=full cargo run {{args}}

# Run the debug build with synthetic demo data and an available update
run-demo-update *args:
    env LEDGER_DEMO=1 LEDGER_DEBUG_UPDATE_AVAILABLE=1 RUST_BACKTRACE=full cargo run {{args}}

run-demo-update-debug *args:
    env LEDGER_DEMO=1 LEDGER_DEBUG_UPDATE_AVAILABLE=1 RUST_LOG='warn,ledger::app=info,ledger::app::popup_diagnostics=debug' RUST_BACKTRACE=full cargo run {{args}}

# Regenerates the README screenshots from demo data (needs a COSMIC session and ImageMagick)
screenshots *args:
    scripts/screenshots.sh {{args}}

# Runs with empty HOME/XDG dirs so provider discovery finds nothing
run-empty-discovery *args:
    rm -rf /tmp/ledger-empty-home /tmp/ledger-empty-config /tmp/ledger-empty-state
    mkdir -p /tmp/ledger-empty-home /tmp/ledger-empty-config /tmp/ledger-empty-state
    env RUST_BACKTRACE=full HOME=/tmp/ledger-empty-home XDG_CONFIG_HOME=/tmp/ledger-empty-config XDG_STATE_HOME=/tmp/ledger-empty-state CARGO_HOME="${CARGO_HOME:-{{home_directory() / '.cargo'}}}" RUSTUP_HOME="${RUSTUP_HOME:-{{home_directory() / '.rustup'}}}" cargo run --release {{args}}

# Adds an applet (Ledger by default) to the start of the COSMIC top panel's right wing
add-to-panel id=appid:
    #!/usr/bin/env bash
    set -euo pipefail
    panel_config="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"
    system_panel_config="/usr/share/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"
    if [[ ! -e "$panel_config" && -e "$system_panel_config" ]]; then
        install -Dm0644 "$system_panel_config" "$panel_config"
    fi
    if [[ -f "$panel_config" ]]; then
        if [[ "$(tr -d '[:space:]' < "$panel_config")" == "None" ]]; then
            printf 'Some(([], ["{{ id }}"]))\n' > "$panel_config"
        else
            sed -i -z \
                -e 's/"{{ id }}"[[:space:]]*,\?[[:space:]]*//g' \
                -e 's/\(Some((\[[^]]*\],[[:space:]]*\[[[:space:]]*\)/\1"{{ id }}",\n    /' \
                "$panel_config"
            grep -qF '"{{ id }}"' "$panel_config" || { echo "error: could not add {{ id }} to $panel_config" >&2; exit 1; }
        fi
    fi

# Removes an applet (Ledger by default) from the COSMIC top panel
remove-from-panel id=appid:
    #!/usr/bin/env bash
    set -euo pipefail
    panel_config="${XDG_CONFIG_HOME:-$HOME/.config}/cosmic/com.system76.CosmicPanel.Panel/v1/plugins_wings"
    if [[ -f "$panel_config" ]]; then
        sed -i -z -e 's/"{{ id }}"[[:space:]]*,\?[[:space:]]*//g' "$panel_config"
    fi

# Installs files
install: build-release add-to-panel
    install -Dm0755 {{ cargo-target-dir / 'release' / name }} {{bin-dst}}
    install -Dm0644 resources/app.desktop {{desktop-dst}}
    install -Dm0644 resources/app.metainfo.xml {{appdata-dst}}
    install -Dm0644 resources/ledger-icon/linux/hicolor/scalable/apps/ledger.svg {{icon-dst}}
    install -Dm0644 resources/ledger-icon/linux/hicolor/symbolic/apps/ledger-symbolic.svg {{icon-symbolic-dst}}

# Installs debug build as `cosmic-applet-ledger-debug` and a separate desktop entry (LEDGER_DEMO) for screenshots
install-demo: build-debug (add-to-panel 'cosmic-applet-ledger-debug')
    install -Dm0755 {{ cargo-target-dir / 'debug' / name }} {{bin-debug-dst}}
    install -Dm0644 resources/app-debug.desktop {{desktop-debug-dst}}

# Removes only the debug install (`install-demo`)
uninstall-demo: (remove-from-panel 'cosmic-applet-ledger-debug')
    rm -f {{bin-debug-dst}} {{desktop-debug-dst}}

# Uninstalls installed files (and debug demo install if present)
uninstall: uninstall-demo remove-from-panel
    rm -f {{bin-dst}} {{desktop-dst}} {{appdata-dst}} {{icon-dst}} {{icon-symbolic-dst}}

# Builds the Flatpak (recreates build-dir; reuses .flatpak-builder cache)
flatpak-build:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git symbolic-ref --quiet --short HEAD)"
    source_dir="$(mktemp -d --tmpdir cosmic-applet-ledger-source.XXXXXX)"
    manifest="$(mktemp --tmpdir cosmic-applet-ledger-flatpak.XXXXXX.json)"
    changed="$(mktemp --tmpdir cosmic-applet-ledger-changed.XXXXXX)"
    deleted="$(mktemp --tmpdir cosmic-applet-ledger-deleted.XXXXXX)"
    trap 'rm -rf "$source_dir" "$manifest" "$changed" "$deleted"' EXIT
    git archive "$branch" | tar -x -C "$source_dir"
    git diff --name-only -z --diff-filter=ACMRTUXB HEAD -- > "$changed"
    git ls-files --others --exclude-standard -z >> "$changed"
    if [[ -s "$changed" ]]; then
      tar --null -T "$changed" -cf - | tar -xf - -C "$source_dir"
    fi
    git diff --name-only -z --diff-filter=D HEAD -- > "$deleted"
    if [[ -s "$deleted" ]]; then
      while IFS= read -r -d '' path; do
        rm -rf "$source_dir/$path"
      done < "$deleted"
    fi
    jq --arg source "$source_dir" --arg cargo_sources "$(pwd)/packaging/cargo-sources.json" '.modules[0].sources = [{"type":"dir","path":$source}, $cargo_sources]' {{ flatpak-manifest }} > "$manifest"
    if [[ -d build-dir && ! -f build-dir/metadata ]]; then
      rm -rf build-dir
    fi
    flatpak-builder \
        --keep-build-dirs \
        --force-clean \
        --default-branch="$branch" \
        build-dir \
        "$manifest"

# Same as flatpak-build; kept as an explicit clean-build entry point
flatpak-build-clean:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git symbolic-ref --quiet --short HEAD)"
    source_dir="$(mktemp -d --tmpdir cosmic-applet-ledger-source.XXXXXX)"
    manifest="$(mktemp --tmpdir cosmic-applet-ledger-flatpak.XXXXXX.json)"
    changed="$(mktemp --tmpdir cosmic-applet-ledger-changed.XXXXXX)"
    deleted="$(mktemp --tmpdir cosmic-applet-ledger-deleted.XXXXXX)"
    trap 'rm -rf "$source_dir" "$manifest" "$changed" "$deleted"' EXIT
    git archive "$branch" | tar -x -C "$source_dir"
    git diff --name-only -z --diff-filter=ACMRTUXB HEAD -- > "$changed"
    git ls-files --others --exclude-standard -z >> "$changed"
    if [[ -s "$changed" ]]; then
      tar --null -T "$changed" -cf - | tar -xf - -C "$source_dir"
    fi
    git diff --name-only -z --diff-filter=D HEAD -- > "$deleted"
    if [[ -s "$deleted" ]]; then
      while IFS= read -r -d '' path; do
        rm -rf "$source_dir/$path"
      done < "$deleted"
    fi
    jq --arg source "$source_dir" --arg cargo_sources "$(pwd)/packaging/cargo-sources.json" '.modules[0].sources = [{"type":"dir","path":$source}, $cargo_sources]' {{ flatpak-manifest }} > "$manifest"
    flatpak-builder \
        --keep-build-dirs \
        --force-clean \
        --default-branch="$branch" \
        build-dir \
        "$manifest"

# Builds the Flatpak, exports to ./repo, and installs for the current user
flatpak-install: flatpak-build add-to-panel
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git symbolic-ref --quiet --short HEAD)"
    mkdir -p repo
    flatpak build-export --update-appstream repo build-dir "$branch"
    flatpak --user install --noninteractive --reinstall "$(pwd)/repo" "{{ appid }}//$branch"
    flatpak --user list --app --columns=application,branch | while IFS=$'\t' read -r app installed_branch; do
      if [[ "$app" == "{{ appid }}" && "$installed_branch" != "$branch" ]]; then
        flatpak --user uninstall --noninteractive "{{ appid }}//$installed_branch" || echo "warning: could not uninstall old Flatpak branch $installed_branch" >&2
      fi
    done
# Export + install only (no flatpak-builder); use after a successful build when nothing needs recompiling
flatpak-install-only: add-to-panel
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git symbolic-ref --quiet --short HEAD)"
    if [[ ! -f build-dir/metadata ]]; then
        echo 'error: no Flatpak in build-dir; run `just flatpak-build` or `just flatpak-install`' >&2
        exit 1
    fi
    mkdir -p repo
    flatpak build-export --update-appstream repo build-dir "$branch"
    flatpak --user install --noninteractive --reinstall "$(pwd)/repo" "{{ appid }}//$branch"
    flatpak --user list --app --columns=application,branch | while IFS=$'\t' read -r app installed_branch; do
      if [[ "$app" == "{{ appid }}" && "$installed_branch" != "$branch" ]]; then
        flatpak --user uninstall --noninteractive "{{ appid }}//$installed_branch" || echo "warning: could not uninstall old Flatpak branch $installed_branch" >&2
      fi
    done

# Runs the installed Flatpak
flatpak-run:
    #!/usr/bin/env bash
    set -euo pipefail
    branch="$(git symbolic-ref --quiet --short HEAD)"
    flatpak run --branch="$branch" {{ appid }}

# Uninstalls the user Flatpak app (undoes `just flatpak-install` / `flatpak-install-only`).
# Removes Flatpak exports (including the `.desktop` entry under the user Flatpak export path). Does not remove `build-dir` / `repo`; use `flatpak-clean` for local build artifacts and app data.
flatpak-uninstall: remove-from-panel
    #!/usr/bin/env bash
    set -euo pipefail
    if flatpak --user info '{{ appid }}' &>/dev/null; then
      flatpak --user uninstall --noninteractive '{{ appid }}'
    else
      echo '{{ appid }} is not installed for the current user; nothing to uninstall.' >&2
    fi
    d="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
    if [[ -d "$d" ]] && command -v update-desktop-database >/dev/null 2>&1; then
      update-desktop-database "$d" || true
    fi

# Remove local Flatpak build outputs, flatpak-builder cache, COSMIC config, and per-app Flatpak data.
# Does not uninstall the app from Flatpak; use `flatpak-uninstall` if needed.
flatpak-clean:
    rm -rf build-dir repo .flatpak-builder
    rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/cosmic/{{ appid }}"
    rm -rf "{{ home_directory() / '.var' / 'app' / appid }}"

# Vendor dependencies locally
vendor:
    mkdir -p .cargo
    cargo vendor --sync Cargo.toml | head -n -1 > .cargo/config.toml
    echo 'directory = "vendor"' >> .cargo/config.toml
    echo >> .cargo/config.toml
    rm -rf .cargo vendor

# Extracts vendored dependencies
vendor-extract:
    rm -rf vendor
    tar pxf vendor.tar

# Regenerate packaging/cargo-sources.json from Cargo.lock and update the commit hash in the manifest.
# Run this after any dependency change or before cutting a release PR to cosmic-flatpak.
update-packaging:
    #!/usr/bin/env bash
    set -euo pipefail
    commit="$(git rev-parse HEAD)"
    echo "Generating cargo-sources.json from Cargo.lock..."
    uv run https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/refs/heads/master/cargo/flatpak-cargo-generator.py \
        Cargo.lock -o packaging/cargo-sources.json
    echo "Updating commit hash in {{ flatpak-manifest }} to $commit..."
    jq --arg commit "$commit" \
        '(.modules[].sources[] | objects | select(.type == "git")) .commit = $commit' \
        {{ flatpak-manifest }} > /tmp/manifest.tmp.json
    mv /tmp/manifest.tmp.json {{ flatpak-manifest }}
    echo "Done. Review packaging/ and copy to cosmic-flatpak/app/{{ appid }}/"

# Bump cargo version, create git commit, and create tag
tag version:
    find -type f -name Cargo.toml -exec sed -i '0,/^version/s/^version.*/version = "{{version}}"/' '{}' \; -exec git add '{}' \;
    cargo check
    cargo clean
    git add Cargo.lock
    git commit -m 'release: {{version}}'
    git tag -a {{version}} -m ''
