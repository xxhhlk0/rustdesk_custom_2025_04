#!/usr/bin/env bash
# Applies the Flutter 3.44-only source/pubspec changes on the fly, in CI only.
#
# Windows arm64 needs Flutter >= 3.44 (the first stable release shipping an arm64 Dart SDK +
# engine), which renamed DialogTheme/TabBarTheme -> *Data and needs newer extended_text/
# google_fonts. Every other platform is still on Flutter 3.24.5, where the old names/versions
# are required, so these changes are kept OUT of the committed sources and applied here instead.
#
# Used by the Windows arm64 build (flutter-build.yml), the iOS build (flutter-build.yml /
# windows-flutter-ci.yml, both on macos-latest) and the dedicated bridge artifact (bridge.yml),
# so they share an identical 3.44 source state -- the generated *.freezed.dart must compile
# against the same Flutter/freezed version the build resolves.
#
# Remove this script (and commit the changes) once upstream bumps Flutter across the board.
#
# Run from the repository root.
#
# The substitutions are done by an embedded Python script, not by sed:
#   - GNU sed uses `sed -i 'expr' file`, BSD sed (macOS) needs `sed -i '' 'expr' file`;
#     on macOS the GNU form makes sed treat the expression as the backup suffix and die with
#     `sed: 1: "flutter/lib/common.dart": invalid command code f`.
#   - the multi-line replacement below relies on a backslash-newline inside the sed
#     replacement, which BSD sed does not interpret the same way.
#   Python 3 is present on every runner we use (ubuntu / macos / windows) and behaves the
#   same everywhere. It also keeps the checked-out CRLF sources intact (windows-11-arm).
set -euo pipefail

readonly NO_MATCHES=0
readonly SINGLE_MATCH=1
readonly THEME_MATCHES=2

has_exact_count() {
  local -r expected_count="$1"
  local -r pattern="$2"
  local -r file="$3"
  local actual_count
  [[ -r "$file" ]] || return 1
  actual_count="$(grep -cF "$pattern" "$file" || true)"
  [[ "$actual_count" -eq "$expected_count" ]]
}

# The target background-color line must directly follow DialogThemeData in the selected range.
has_dialog_background_in_theme_range() {
  local -r start_pattern="$1"
  local -r end_pattern="$2"
  local -r target_pattern="$3"
  local -r file="$4"
  awk -v start_pattern="$start_pattern" \
    -v end_pattern="$end_pattern" \
    -v target_pattern="$target_pattern" '
      index($0, start_pattern) {
        in_theme = 1
        next
      }
      in_theme && index($0, end_pattern) {
        exit
      }
      in_theme && index($0, "dialogTheme: DialogThemeData(") {
        if (getline > 0) {
          line = $0
          sub(/\r$/, "", line)
          sub(/^[[:space:]]+/, "", line)
          matched = line == target_pattern
        }
        exit
      }
      END {
        exit matched ? 0 : 1
      }
    ' "$file"
}

validate_patch_inputs() {
  if [[ ! -f flutter/lib/common.dart || ! -r flutter/lib/common.dart ]]; then
    echo "Flutter 3.44 source patch input is missing or unreadable: flutter/lib/common.dart" >&2
    return 1
  fi
  if [[ ! -f flutter/pubspec.yaml || ! -r flutter/pubspec.yaml ]]; then
    echo "Flutter 3.44 source patch input is missing or unreadable: flutter/pubspec.yaml" >&2
    return 1
  fi
}

is_complete_patch_state() {
  has_exact_count "$THEME_MATCHES" 'dialogTheme: DialogThemeData(' flutter/lib/common.dart &&
    has_exact_count "$THEME_MATCHES" 'tabBarTheme: const TabBarThemeData(' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'backgroundColor: Colors.white,' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'backgroundColor: Color(0xFF18191E),' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'extended_text: 15.0.2' flutter/pubspec.yaml &&
    has_exact_count "$SINGLE_MATCH" 'google_fonts: ^8.1.0' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'dialogTheme: DialogTheme(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'tabBarTheme: const TabBarTheme(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'extended_text: 14.0.0' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'google_fonts: ^6.2.1' flutter/pubspec.yaml &&
    has_dialog_background_in_theme_range 'static ThemeData lightTheme = ThemeData(' \
      'static ThemeData darkTheme = ThemeData(' 'backgroundColor: Colors.white,' \
      flutter/lib/common.dart &&
    has_dialog_background_in_theme_range 'static ThemeData darkTheme = ThemeData(' \
      'scrollbarTheme: scrollbarThemeDark,' 'backgroundColor: Color(0xFF18191E),' \
      flutter/lib/common.dart
}

is_unpatched_state() {
  has_exact_count "$THEME_MATCHES" 'dialogTheme: DialogTheme(' flutter/lib/common.dart &&
    has_exact_count "$THEME_MATCHES" 'tabBarTheme: const TabBarTheme(' flutter/lib/common.dart &&
    has_exact_count "$SINGLE_MATCH" 'extended_text: 14.0.0' flutter/pubspec.yaml &&
    has_exact_count "$SINGLE_MATCH" 'google_fonts: ^6.2.1' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'dialogTheme: DialogThemeData(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'tabBarTheme: const TabBarThemeData(' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'backgroundColor: Colors.white,' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'backgroundColor: Color(0xFF18191E),' flutter/lib/common.dart &&
    has_exact_count "$NO_MATCHES" 'extended_text: 15.0.2' flutter/pubspec.yaml &&
    has_exact_count "$NO_MATCHES" 'google_fonts: ^8.1.0' flutter/pubspec.yaml
}

if ! validate_patch_inputs; then
  exit 1
fi

if is_complete_patch_state; then
  echo "Flutter 3.44 source patches already applied."
  git --no-pager diff -- flutter/lib/common.dart flutter/pubspec.yaml
  exit 0
fi

if ! is_unpatched_state; then
  echo "Flutter 3.44 source patches are partially applied or their anchors have drifted." >&2
  exit 1
fi

# ThemeData API renames (Flutter 3.27+) + dependency bumps + the dialog background colors.
python3 - <<'PY'
import sys

COMMON = 'flutter/lib/common.dart'
PUBSPEC = 'flutter/pubspec.yaml'

NEEDLE = 'dialogTheme: DialogThemeData('
WHITE = '      backgroundColor: Colors.white,'
DARK = '      backgroundColor: Color(0xFF18191E),'


def read(path):
    with open(path, 'rb') as f:
        return f.read().decode('utf-8')


def write(path, text):
    with open(path, 'wb') as f:
        f.write(text.encode('utf-8'))


def eol_of(text):
    return '\r\n' if '\r\n' in text else '\n'


def insert_after_needle(lines, start_marker, end_marker, needle, insert_text, eol):
    """Inside [start_marker, end_marker] (both inclusive), append insert_text on a new
    line right after the first occurrence of needle on a line. Mirrors the previous
    `sed '/start/,/end/s/needle/needle\\\n      <insert>/'` behaviour."""
    out = []
    in_range = False
    for line in lines:
        body = line.rstrip('\r\n')
        term = line[len(body):]
        if in_range and needle in body:
            pos = body.index(needle)
            body = body[:pos] + needle + eol + insert_text + body[pos + len(needle):]
            out.append(body + term)
            continue
        out.append(line)
        if not in_range:
            if start_marker in body:
                in_range = True
        elif end_marker in body:
            in_range = False
    return out


common = read(COMMON)
eol = eol_of(common)

# 1) ThemeData API renames (Flutter 3.27+)
common = common.replace('dialogTheme: DialogTheme(', 'dialogTheme: DialogThemeData(')
common = common.replace('tabBarTheme: const TabBarTheme(', 'tabBarTheme: const TabBarThemeData(')

# 2) dialog background colors, per theme block
lines = common.splitlines(keepends=True)
lines = insert_after_needle(
    lines,
    'static ThemeData lightTheme = ThemeData(',
    'static ThemeData darkTheme = ThemeData(',
    NEEDLE, WHITE, eol)
lines = insert_after_needle(
    lines,
    'static ThemeData darkTheme = ThemeData(',
    'scrollbarTheme: scrollbarThemeDark,',
    NEEDLE, DARK, eol)
common = ''.join(lines)
write(COMMON, common)

# 3) Dependency bumps required by the newer Dart/Flutter
pubspec = read(PUBSPEC)
pubspec = pubspec.replace('extended_text: 14.0.0', 'extended_text: 15.0.2')
pubspec = pubspec.replace('google_fonts: ^6.2.1', 'google_fonts: ^8.1.0')
write(PUBSPEC, pubspec)

print('Flutter 3.44 source patches applied.')
PY

# Fail loudly if any expected substitution did not produce the complete state.
if ! is_complete_patch_state; then
  echo "Flutter 3.44 source patches did not produce the expected state." >&2
  exit 1
fi

git --no-pager diff -- flutter/lib/common.dart flutter/pubspec.yaml
