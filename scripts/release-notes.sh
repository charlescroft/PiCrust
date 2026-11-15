#!/usr/bin/env bash
# Builds the complete GitHub release body for a tag.
#
# Usage: scripts/release-notes.sh v1.1.0 [assets-dir]
#   GITHUB_REPOSITORY / GITHUB_REF_NAME are used when set by Actions.
#
# The notes always contain: the changelog section for the tag, a download
# table whose links point at the release assets, install instructions for all
# three platforms, and checksum verification guidance.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CHANGELOG="$REPO_ROOT/CHANGELOG.md"

TAG="${1:-${GITHUB_REF_NAME:-}}"
if [[ -z "$TAG" ]]; then
  echo "usage: $0 <tag> [assets-dir]" >&2
  exit 1
fi
VERSION="${TAG#v}"
ASSETS_DIR="${2:-${ASSETS_DIR:-}}"
REPOSITORY="${GITHUB_REPOSITORY:-charlescroft/picrust}"
RELEASE_URL="https://github.com/$REPOSITORY/releases/download/$TAG"
RELEASE_PAGE="https://github.com/$REPOSITORY/releases/tag/$TAG"

SECTION="$(awk -v ver="$VERSION" '
  $0 ~ "^## \\[" ver "\\]" { printing = 1 }
  printing && /^## \[/ && $0 !~ "^## \\[" ver "\\]" { exit }
  printing { print }
' "$CHANGELOG")"

if [[ -z "${SECTION//[[:space:]]/}" ]]; then
  echo "error: CHANGELOG.md has no '## [$VERSION]' section; refusing to publish incomplete release notes" >&2
  exit 1
fi

printf '%s\n\n' "$SECTION"

printf '## Downloads\n\n'
printf '| Platform | Artifact | Notes |\n'
printf '| --- | --- | --- |\n'

if [[ -n "$ASSETS_DIR" && -d "$ASSETS_DIR" ]]; then
  while IFS= read -r file; do
    name="$(basename "$file")"
    case "$name" in
      *.exe) platform="Windows x64"; note="Standalone executable (portable, no installer)" ;;
      *windows*.zip) platform="Windows x64"; note="Portable bundle (sidecar + resources)" ;;
      *macOS*.zip|*macos*.zip) platform="macOS"; note="Zipped .app bundle" ;;
      *.AppImage) platform="Linux x86_64"; note="Standalone AppImage" ;;
      SHA256SUMS*) continue ;;
      *) platform="Other"; note="" ;;
    esac
    printf '| %s | [%s](%s/%s) | %s |\n' "$platform" "$name" "$RELEASE_URL" "$name" "$note"
  done < <(find "$ASSETS_DIR" -maxdepth 1 -type f | sort)
else
  printf '| Windows x64 | PiCrust-windows-x64-%s.exe | Standalone executable |\n' "$VERSION"
  printf '| Windows x64 | PiCrust-windows-x64-%s.zip | Portable bundle |\n' "$VERSION"
  printf '| macOS | PiCrust-macOS-<arch>-%s.zip | Zipped .app bundle |\n' "$VERSION"
  printf '| Linux x86_64 | PiCrust-Linux-x86_64-%s.AppImage | Standalone AppImage |\n' "$VERSION"
fi

cat <<EOF

## Install

- **Windows**: download the \`.exe\` and run it. The portable \`.zip\` contains the
  bundled sidecar and resources and can be extracted anywhere.
- **macOS**: unzip \`PiCrust-macOS-<arch>-${VERSION}.zip\`, move \`PiCrust.app\` to
  \`/Applications\`, then right-click → Open the first time (the build is
  ad-hoc signed and not notarized).
- **Linux**: \`chmod +x PiCrust-Linux-x86_64-${VERSION}.AppImage\` and run it.
  A FUSE-capable desktop is required; otherwise run with \`--appimage-extract-and-run\`.

## Verify

Download \`SHA256SUMS-${VERSION}.txt\` and run:

\`\`\`bash
sha256sum -c SHA256SUMS-${VERSION}.txt
\`\`\`

## Links

- Full changelog: [CHANGELOG.md](https://github.com/$REPOSITORY/blob/main/CHANGELOG.md)
- All release notes: [$TAG]($RELEASE_PAGE)
EOF
