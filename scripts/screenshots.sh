#!/usr/bin/env bash
# Regenerates the README screenshots from demo data (LEDGER_DEMO=1).
#
# Runs a debug build with LEDGER_DEBUG_POPUP_WINDOW so each popup page opens
# in a plain window, takes a full-screen shot with cosmic-screenshot, finds the
# window by its accent-coloured border (retrying until it opens with focus),
# and crops the popup card out. Settings and accounts live in throwaway XDG
# folders, so your real Ledger config and accounts are never read or touched.
#
# Needs: a COSMIC session (dark theme), cosmic-screenshot, ImageMagick 7.
# Keep other windows from covering the centre of the screen while it runs.
#
# Usage: just screenshots   (or: scripts/screenshots.sh [out-dir])
# Re-shoot only some pages: LEDGER_SHOT_ONLY="accounts ring-fish" just screenshots

set -euo pipefail

out_dir="${1:-resources/screenshots}"
bin="./target/debug/cosmic-applet-ledger"
# Window borders use the COSMIC accent colour while the window has focus.
# Set LEDGER_SHOT_ACCENT="r,g,b" if your accent is not the default blue.
accent="${LEDGER_SHOT_ACCENT:-122,162,247}"
# Popup card background in the COSMIC dark theme.
card_bg="27,27,27"

work="$(mktemp -d)"
trap 'pkill -f "^${bin}" 2>/dev/null || true; rm -rf "$work"' EXIT

cfg="$work/config/cosmic/io.github.chefn3ff.cosmic-applet-ledger/v600"
mkdir -p "$cfg" "$work/state" "$work/cache" "$out_dir"

set_cfg() { printf '%s\n' "$2" > "$cfg/$1"; }

# Reads "magick ... txt:-" output and prints where each thin (at most 6 px)
# run of accent pixels starts, along coordinate $1 (1 = x, 2 = y).
thin_accent_runs() {
    awk -v a="$accent" -v axis="$1" -F '[:( ]+' '
        index($0, "(" a ")") || index($0, "(" a ",") {
            split($1, p, ",")
            v = p[axis] + 0
            if (started && v == last + 1) {
                len++
            } else {
                if (started && len <= 6) print start
                start = v; len = 1; started = 1
            }
            last = v
        }
        END { if (started && len <= 6) print start }'
}

# Finds the focused popup window by its thin accent border and prints
# "x y w h" for the area inside it, below the server-drawn title bar.
window_box() {
    local shot="$1" width height col row v
    local top="" bottom="" left="" right=""
    width=$(magick identify -format '%w' "$shot")
    height=$(magick identify -format '%h' "$shot")
    # The window opens centred. Sample left of centre, so a tiling seam on the
    # centre line (another window's border) is never mistaken for it.
    col=$((width / 2 - 150))
    while read -r v; do
        if [[ -z "$top" ]]; then
            top=$v
        elif (( v > top + 100 )); then
            bottom=$v
            break
        fi
    done < <(magick "$shot" -crop "1x${height}+${col}+0" +repage txt:- | thin_accent_runs 2)
    [[ -n "$top" && -n "$bottom" ]] || return 1
    row=$((top + 60))
    while read -r v; do
        (( v > width / 2 - 400 && v < width / 2 + 400 )) || continue
        [[ -z "$left" ]] && left=$v
        right=$v
    done < <(magick "$shot" -crop "${width}x1+0+${row}" +repage txt:- | thin_accent_runs 1)
    [[ -n "$left" && -n "$right" ]] || return 1
    (( right - left > 200 )) || return 1
    local inset=4 title=48
    echo "$((left + inset)) $((top + title)) $((right - left - 2 * inset)) $((bottom - top - title - inset))"
}

# Height of the popup card inside a cropped window. The window is see-through
# below the card, so take the lowest row where the card's left margin still
# shows the card background, less a few rows to drop its rounded bottom edge.
card_height() {
    local img="$1" h
    h=$(magick identify -format '%h' "$img")
    magick "$img" -crop "1x${h}+16+0" +repage txt:- |
        awk -v bg="$card_bg" -F '[:( ]+' '
            index($0, "(" bg ")") || index($0, "(" bg ",") { split($1, p, ","); y = p[2] + 0 }
            END { print y - 8 }'
}

shoot() {
    local name="$1" page="$2" shot="" box="" attempt x y w h
    if [[ -n "${LEDGER_SHOT_ONLY:-}" && " ${LEDGER_SHOT_ONLY} " != *" $name "* ]]; then
        return 0
    fi
    for attempt in 1 2 3 4 5; do
        pkill -f "^${bin}" 2>/dev/null || true
        sleep 1
        (XDG_CONFIG_HOME="$work/config" XDG_STATE_HOME="$work/state" XDG_CACHE_HOME="$work/cache" \
            LEDGER_DEMO=1 LEDGER_DEBUG_POPUP_WINDOW="$page" setsid "$bin" >"$work/$name.log" 2>&1 &)
        sleep 5
        rm -f "$work"/Screenshot_*.png
        cosmic-screenshot --interactive=false --notify=false --save-dir "$work" >/dev/null
        shot=$(ls "$work"/Screenshot_*.png)
        # No accent border means the window opened without focus: try again.
        if box=$(window_box "$shot"); then
            break
        fi
        echo "  $name: window not found (attempt $attempt)" >&2
        rm -f "$shot"
        box=""
    done
    if [[ -z "$box" ]]; then
        echo "  $name: giving up" >&2
        return 1
    fi
    read -r x y w h <<<"$box"
    magick "$shot" -crop "${w}x${h}+${x}+${y}" +repage "$work/window.png"
    h=$(card_height "$work/window.png")
    # Keep only the card, trim the empty background around short pages, and
    # pad back evenly so every image has the same margins.
    # The 8 px inset on the other sides drops the card's rounded corners too.
    magick "$work/window.png" -crop "$((w - 16))x$((h - 8))+8+8" +repage \
        -bordercolor "rgb(${card_bg})" -fuzz 2% -trim +repage -border 24 \
        "$out_dir/$name.png"
    rm -f "$shot" "$work/window.png"
    echo "  $out_dir/$name.png"
}

cargo build --quiet
failed=()

# Provider pages, without header rings.
set_cfg elevenlabs_ring_enabled false
set_cfg fish_ring_enabled false
for provider in antigravity claude codex copilot cursor elevenlabs fish gemini grok kimi minimax open_code_go open_router zai; do
    set_cfg selected_provider "$provider"
    shoot "provider-${provider//_/-}" 1 || failed+=("provider-${provider//_/-}")
done

# Settings, Switch Provider and an accounts page.
set_cfg selected_provider claude
shoot settings settings || failed+=(settings)
shoot switch-provider switch || failed+=(switch-provider)
shoot accounts accounts || failed+=(accounts)

# Header credit rings: both on, once showing each service.
set_cfg elevenlabs_ring_enabled true
set_cfg fish_ring_enabled true
set_cfg selected_provider codex
set_cfg header_ring_provider elevenlabs
shoot ring-elevenlabs 1 || failed+=(ring-elevenlabs)
set_cfg header_ring_provider fish
shoot ring-fish 1 || failed+=(ring-fish)

# README hero: three pages side by side, top-aligned, on a transparent
# background so it sits well on light and dark GitHub themes. One image
# scales down cleanly on a phone instead of stacking three tall ones.
if [[ -f "$out_dir/provider-claude.png" && -f "$out_dir/provider-elevenlabs.png" && -f "$out_dir/ring-fish.png" ]]; then
    magick "$out_dir/provider-claude.png" "$out_dir/provider-elevenlabs.png" "$out_dir/ring-fish.png" +repage \
        -bordercolor none -border 12x0 -background none -gravity north +append +repage "$out_dir/hero.png"
    echo "  $out_dir/hero.png"
fi

if (( ${#failed[@]} )); then
    echo "failed: ${failed[*]} (run again with the screen centre clear)" >&2
    exit 1
fi
echo "done"
