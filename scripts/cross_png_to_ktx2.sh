#!/usr/bin/env bash
set -euo pipefail

# =========================================================
# cross_png_to_ktx2.sh
#
# Convert a single cubemap image (h-cross / v-cross / strips) into a KTX2 cubemap.
# - Crops faces with ImageMagick
# - Optionally fixes per-face orientation (rotate/flip/swap)
# - Packages into KTX2 with KTX-Software `ktx create`
# - Optional encoding (uastc / basis-lz)
#
# Usage:
#   ./cross_png_to_ktx2.sh <input.png> <output.ktx2>
#
# Env:
#   LOG_LEVEL=debug|info|warn|error     (default: info)
#   ENCODE=none|uastc|basis-lz         (default: none)
#   UASTC_QUALITY=0..4                 (default: 2)
#   UASTC_RDO=0|1                      (default: 0)
#   UASTC_RDO_L=0.001..10              (default: 1.0)
#   QLEVEL=1..255                      (basis-lz quality, default: 128)
#   CLEVEL=0..6                        (basis-lz compression, default: 1)
#   ZSTD=1..22                         (optional supercompression, default: unset)
#   ZLIB=1..9                          (optional supercompression, default: unset)
#   MIPMAP=0|1                         (default: 1)
#
# Orientation overrides (space-separated):
#   FACE_ROTATE="py:90 ny:-90 nz:180"      (degrees: -360..360)
#   FACE_FLIP="nx:h py:v nz:hv"            (h=flop, v=flip, hv=flip+flop)
#   FACE_SWAP="pz:nz px:nx"                (swap face images)
#
# Debug:
#   DEBUG_PREVIEW=0|1                 (default: 0) writes a preview cross png to temp dir
#
# Notes:
# - Cubemap face order for `ktx create --cubemap` is: +X -X +Y -Y +Z -Z
#   We write: px nx py ny pz nz
# =========================================================

# ---------------- Logging ----------------
LOG_LEVEL="${LOG_LEVEL:-info}" # debug|info|warn|error
_timestamp() { date +"%H:%M:%S"; }

_level_num() {
  case "${1:-info}" in
    debug) echo 0 ;;
    info)  echo 1 ;;
    warn)  echo 2 ;;
    error) echo 3 ;;
    *)     echo 1 ;;
  esac
}

_should_log() {
  local want="$(_level_num "$1")"
  local cur="$(_level_num "$LOG_LEVEL")"
  (( want >= cur ))
}

log() {
  local lvl="$1"; shift
  _should_log "$lvl" || return 0
  printf "[%s] %-5s %s\n" "$(_timestamp)" "$lvl" "$*" >&2
}

die() { log error "$*"; exit 1; }

run() {
  if _should_log debug; then log debug "RUN: $*"; fi
  "$@"
}

on_err() {
  local ec=$?
  log error "Failed at line $1 (exit=$ec): ${BASH_COMMAND}"
  exit "$ec"
}
trap 'on_err $LINENO' ERR

# ---------------- Args ----------------
if [[ $# -ne 2 ]]; then
  cat >&2 <<EOF
Usage: $0 <input_cubemap_png> <output.ktx2>

Env:
  LOG_LEVEL=debug|info|warn|error     (default: info)
  ENCODE=none|uastc|basis-lz          (default: none)
  UASTC_QUALITY=0..4                  (default: 2)
  UASTC_RDO=0|1                       (default: 0)
  UASTC_RDO_L=0.001..10               (default: 1.0)
  QLEVEL=1..255                       (basis-lz, default: 128)
  CLEVEL=0..6                         (basis-lz, default: 1)
  ZSTD=1..22                          (optional)
  ZLIB=1..9                           (optional)
  MIPMAP=0|1                          (default: 1)

  FACE_ROTATE="py:90 ny:-90 nz:180"
  FACE_FLIP="nx:h py:v nz:hv"
  FACE_SWAP="pz:nz px:nx"

  DEBUG_PREVIEW=0|1                   (default: 0)
EOF
  exit 1
fi

INPUT="$1"
OUTPUT="$2"
INPUT_ABS="$(realpath "$INPUT")"
OUTPUT_ABS="$(realpath -m "$OUTPUT")"

log info "Input : $INPUT_ABS"
log info "Output: $OUTPUT_ABS"
run mkdir -p "$(dirname "$OUTPUT_ABS")"

# ---------------- Tool detection ----------------
# ImageMagick: use "magick" if available; else "convert". Use "identify" for metadata.
MAGICK_BIN=""
if command -v magick >/dev/null 2>&1; then
  MAGICK_BIN="magick"
elif command -v convert >/dev/null 2>&1; then
  MAGICK_BIN="convert"
else
  die "ImageMagick not found. Install: sudo dnf install ImageMagick"
fi
command -v identify >/dev/null 2>&1 || die "'identify' not found (ImageMagick)."

command -v ktx >/dev/null 2>&1 || die "'ktx' not found in PATH."

log info "Using crop tool: $MAGICK_BIN"
log info "Using ktx      : $(command -v ktx)"

# Temp workspace
TMPDIR="$(mktemp -d)"
log info "Temp dir: $TMPDIR"
trap 'log info "Cleaning up $TMPDIR"; rm -rf "$TMPDIR"' EXIT

# ---------------- Helpers ----------------
get_dims() {
  local out
  out="$(identify -ping -format "%w %h" "$INPUT_ABS" 2>/dev/null || true)"
  [[ "$out" =~ ^[0-9]+\ [0-9]+$ ]] || die "Failed to read image size via identify: '$INPUT_ABS'"
  echo "$out"
}

is_pow2() { local n="$1"; (( n > 0 && ( (n & (n-1)) == 0 ) )); }

crop_face() {
  local name="$1" x="$2" y="$3" s="$4"
  local out="$TMPDIR/${name}.png"

  log info "Crop $name: ${s}x${s}+${x}+${y}"
  if [[ "$MAGICK_BIN" == "magick" ]]; then
    run magick "$INPUT_ABS" -crop "${s}x${s}+${x}+${y}" +repage "$out"
  else
    run convert "$INPUT_ABS" -crop "${s}x${s}+${x}+${y}" +repage "$out"
  fi
}

swap_faces() {
  # FACE_SWAP="pz:nz px:nx"
  local spec="${FACE_SWAP:-}"
  [[ -z "$spec" ]] && return 0

  for pair in $spec; do
    local a="${pair%%:*}"
    local b="${pair#*:}"
    [[ -f "$TMPDIR/$a.png" ]] || die "FACE_SWAP refers to missing face '$a'"
    [[ -f "$TMPDIR/$b.png" ]] || die "FACE_SWAP refers to missing face '$b'"
    log info "Swap faces: $a <-> $b"
    run mv "$TMPDIR/$a.png" "$TMPDIR/$a.png.__tmp__"
    run mv "$TMPDIR/$b.png" "$TMPDIR/$a.png"
    run mv "$TMPDIR/$a.png.__tmp__" "$TMPDIR/$b.png"
  done
}

apply_ops_one() {
  local f="$1"
  local path="$TMPDIR/$f.png"

  # rotations: "py:90 ny:-90"
  local spec="${FACE_ROTATE:-}"
  if [[ -n "$spec" ]]; then
    for op in $spec; do
      local name="${op%%:*}" deg="${op#*:}"
      [[ "$name" == "$f" ]] || continue
      log info "Rotate $f by ${deg}°"
      if [[ "$MAGICK_BIN" == "magick" ]]; then
        run magick "$path" -rotate "$deg" "$path"
      else
        run convert "$path" -rotate "$deg" "$path"
      fi
    done
  fi

  # flips: "nx:h py:v nz:hv"
  spec="${FACE_FLIP:-}"
  if [[ -n "$spec" ]]; then
    for op in $spec; do
      local name="${op%%:*}" kind="${op#*:}"
      [[ "$name" == "$f" ]] || continue
      log info "Flip $f: $kind"
      case "$kind" in
        h)
          if [[ "$MAGICK_BIN" == "magick" ]]; then run magick "$path" -flop "$path"; else run convert "$path" -flop "$path"; fi
          ;;
        v)
          if [[ "$MAGICK_BIN" == "magick" ]]; then run magick "$path" -flip "$path"; else run convert "$path" -flip "$path"; fi
          ;;
        hv|vh)
          if [[ "$MAGICK_BIN" == "magick" ]]; then run magick "$path" -flip -flop "$path"; else run convert "$path" -flip -flop "$path"; fi
          ;;
        *)
          die "Unknown FACE_FLIP kind '$kind' (use h|v|hv)"
          ;;
      esac
    done
  fi
}

debug_preview() {
  [[ "${DEBUG_PREVIEW:-0}" == "1" ]] || return 0
  local FACE="$1"
  local out="$TMPDIR/preview_cross.png"
  log info "Writing preview cross: $out"

  if [[ "$MAGICK_BIN" == "magick" ]]; then
    run magick -size $((4*FACE))x$((3*FACE)) xc:black \
      "$TMPDIR/py.png" -geometry +$((1*FACE))+$((0*FACE)) -composite \
      "$TMPDIR/nx.png" -geometry +$((0*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/pz.png" -geometry +$((1*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/px.png" -geometry +$((2*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/nz.png" -geometry +$((3*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/ny.png" -geometry +$((1*FACE))+$((2*FACE)) -composite \
      "$out"
  else
    run convert -size $((4*FACE))x$((3*FACE)) xc:black \
      "$TMPDIR/py.png" -geometry +$((1*FACE))+$((0*FACE)) -composite \
      "$TMPDIR/nx.png" -geometry +$((0*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/pz.png" -geometry +$((1*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/px.png" -geometry +$((2*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/nz.png" -geometry +$((3*FACE))+$((1*FACE)) -composite \
      "$TMPDIR/ny.png" -geometry +$((1*FACE))+$((2*FACE)) -composite \
      "$out"
  fi
}

# ---------------- Detect layout + crop ----------------
read -r W H < <(get_dims)
log info "Input size: ${W}x${H}"

FACE=0
LAYOUT=""

# Horizontal cross: 4N x 3N
if (( W % 4 == 0 )) && (( H % 3 == 0 )) && (( W / 4 == H / 3 )); then
  FACE=$((W / 4))
  LAYOUT="hcross"
  log info "Detected layout: $LAYOUT (face=${FACE}px)"
  #        [py]
  # [nx] [pz] [px] [nz]
  #        [ny]
  crop_face py $((1*FACE)) $((0*FACE)) "$FACE"
  crop_face nx $((0*FACE)) $((1*FACE)) "$FACE"
  crop_face pz $((1*FACE)) $((1*FACE)) "$FACE"
  crop_face px $((2*FACE)) $((1*FACE)) "$FACE"
  crop_face nz $((3*FACE)) $((1*FACE)) "$FACE"
  crop_face ny $((1*FACE)) $((2*FACE)) "$FACE"

# Vertical cross: 3N x 4N
elif (( W % 3 == 0 )) && (( H % 4 == 0 )) && (( W / 3 == H / 4 )); then
  FACE=$((W / 3))
  LAYOUT="vcross"
  log info "Detected layout: $LAYOUT (face=${FACE}px)"
  #        [py]
  # [nx] [pz] [px]
  #        [ny]
  #        [nz]
  crop_face py $((1*FACE)) $((0*FACE)) "$FACE"
  crop_face nx $((0*FACE)) $((1*FACE)) "$FACE"
  crop_face pz $((1*FACE)) $((1*FACE)) "$FACE"
  crop_face px $((2*FACE)) $((1*FACE)) "$FACE"
  crop_face ny $((1*FACE)) $((2*FACE)) "$FACE"
  crop_face nz $((1*FACE)) $((3*FACE)) "$FACE"

# Horizontal strip: 6N x N
elif (( H > 0 )) && (( W % 6 == 0 )) && (( W / 6 == H )); then
  FACE=$H
  LAYOUT="hstrip"
  log info "Detected layout: $LAYOUT (face=${FACE}px)"
  # Assumption: +X -X +Y -Y +Z -Z (common but not universal)
  crop_face px $((0*FACE)) 0 "$FACE"
  crop_face nx $((1*FACE)) 0 "$FACE"
  crop_face py $((2*FACE)) 0 "$FACE"
  crop_face ny $((3*FACE)) 0 "$FACE"
  crop_face pz $((4*FACE)) 0 "$FACE"
  crop_face nz $((5*FACE)) 0 "$FACE"

# Vertical strip: N x 6N
elif (( W > 0 )) && (( H % 6 == 0 )) && (( H / 6 == W )); then
  FACE=$W
  LAYOUT="vstrip"
  log info "Detected layout: $LAYOUT (face=${FACE}px)"
  crop_face px 0 $((0*FACE)) "$FACE"
  crop_face nx 0 $((1*FACE)) "$FACE"
  crop_face py 0 $((2*FACE)) "$FACE"
  crop_face ny 0 $((3*FACE)) "$FACE"
  crop_face pz 0 $((4*FACE)) "$FACE"
  crop_face nz 0 $((5*FACE)) "$FACE"
else
  die "Unsupported layout: ${W}x${H}. Expected 4N×3N, 3N×4N, 6N×N, or N×6N."
fi

if is_pow2 "$FACE"; then
  log info "Face size is power-of-two: $FACE"
else
  log warn "Face size is NOT power-of-two: $FACE (still OK; mipmaps will work)"
fi

# ensure faces exist
for f in px nx py ny pz nz; do
  [[ -f "$TMPDIR/$f.png" ]] || die "Missing face '$f' after crop"
done
log info "Cropped 6 faces."

# ---------------- Optional swaps / transforms ----------------
swap_faces
for f in px nx py ny pz nz; do
  apply_ops_one "$f"
done
debug_preview "$FACE"

# ---------------- Create KTX2 cubemap ----------------
MIPMAP="${MIPMAP:-1}"
ENCODE="${ENCODE:-none}"
UASTC_QUALITY="${UASTC_QUALITY:-2}"
UASTC_RDO="${UASTC_RDO:-0}"
UASTC_RDO_L="${UASTC_RDO_L:-1.0}"
QLEVEL="${QLEVEL:-128}"
CLEVEL="${CLEVEL:-1}"

ENCODE_ARGS=()
case "$ENCODE" in
  none|"") ;;
  uastc)
    ENCODE_ARGS+=(--encode uastc --uastc-quality "$UASTC_QUALITY")
    if [[ "$UASTC_RDO" == "1" ]]; then
      ENCODE_ARGS+=(--uastc-rdo --uastc-rdo-l "$UASTC_RDO_L")
    fi
    ;;
  basis-lz|basislz|etc1s)
    ENCODE_ARGS+=(--encode basis-lz --qlevel "$QLEVEL" --clevel "$CLEVEL")
    ;;
  *)
    die "Unknown ENCODE='$ENCODE' (use none|uastc|basis-lz)"
    ;;
esac

SUPERCOMP_ARGS=()
if [[ -n "${ZSTD:-}" ]]; then SUPERCOMP_ARGS+=(--zstd "$ZSTD"); fi
if [[ -n "${ZLIB:-}" ]]; then SUPERCOMP_ARGS+=(--zlib "$ZLIB"); fi

CREATE_ARGS=(
  create
  --cubemap
  --format R8G8B8A8_SRGB
)

if [[ "$MIPMAP" == "1" ]]; then
  CREATE_ARGS+=(--generate-mipmap)
fi

log info "Creating KTX2 cubemap (ENCODE=$ENCODE, MIPMAP=$MIPMAP)..."
run ktx "${CREATE_ARGS[@]}" \
  "${ENCODE_ARGS[@]}" \
  "${SUPERCOMP_ARGS[@]}" \
  "$TMPDIR/px.png" \
  "$TMPDIR/nx.png" \
  "$TMPDIR/py.png" \
  "$TMPDIR/ny.png" \
  "$TMPDIR/pz.png" \
  "$TMPDIR/nz.png" \
  "$OUTPUT_ABS"

log info "Done."
echo "OK: wrote $OUTPUT_ABS (layout=$LAYOUT, face=${FACE}px, encode=$ENCODE, mipmap=$MIPMAP)"