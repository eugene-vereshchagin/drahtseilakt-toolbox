#!/usr/bin/env bash
# Packs one built toolbox binary for one target into dist/ (run by .github/workflows/toolbox-release.yml, on Linux for
# every target; it is only archiving, so a Windows or macOS binary is packed here as well as a Linux one).
#
#   scripts/ci-package.sh OS ARCH BINARY VERSION
#     OS: linux | windows | macos | freebsd;  ARCH: x86_64 | aarch64;  VERSION: 1.2.3 or git-abc1234
#   Environment: GAME_DIR (the game's checkout; default ../drahtseilakt: its assets/fonts are the fonts the window reads)
#
# Outputs (dist/):  linux    portable .tar.gz, .deb, .rpm
#                   windows  portable .zip, installer .exe (NSIS; needs makensis)
#                   macos    portable .tar.gz (not signed)      freebsd  portable .tar.gz
# Every archive holds the program, fonts/ (the game's OFL fonts, with their licence texts: the window reads them
# from fonts/ beside the program), realesrgan/ where a pinned build exists, README.txt, LICENSE, THIRD-PARTY.md.
# Real-ESRGAN: Linux x86_64, Windows x86_64, macOS (universal); pins and SHA-256 in third-party.txt, downloaded into
# third_party/download/ when not there and checked; elsewhere it is left out and README.txt says so.
set -euo pipefail

die() { echo "ci-package: $*" >&2; exit 1; }
[ $# -eq 4 ] || die "usage: ci-package.sh OS ARCH BINARY VERSION"
os=$1 arch=$2 bin=$3 version=$4
here=$(cd "$(dirname "$0")/.." && pwd)
game=${GAME_DIR:-$here/../drahtseilakt}
dist=${DIST_DIR:-$here/dist}
[ -f "$bin" ] || die "no binary $bin"
case "$os" in linux | windows | macos | freebsd) ;; *) die "unknown OS $os" ;; esac
case "$arch" in x86_64 | aarch64) ;; *) die "unknown architecture $arch" ;; esac

# package version: a tag as it is; a dev build (git-abc1234) as <Cargo version>~gitabc1234 (valid for deb and rpm)
cargo_version=$(awk -F'"' '/^version *=/ { print $2; exit }' "$here/Cargo.toml")
if [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then pkgver=$version; else pkgver="${cargo_version}~${version//-/}"; fi

name=drahtseilakt-toolbox
exe=$name; [ "$os" != windows ] || exe=$name.exe
folder="$name-$version-$os-$arch"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
tree="$work/$folder"
mkdir -p "$tree/fonts" "$dist"

# --- the program, fonts, licences ---------------------------------------------------------------------------------
install -m 755 "$bin" "$tree/$exe"
for f in NotoSans-Regular.ttf NotoSansSC-Regular.otf Tektur-Variable.ttf OFL.txt OFL-NotoSans.txt OFL-Tektur.txt; do
    [ -f "$game/assets/fonts/$f" ] || die "the game's checkout has no assets/fonts/$f ($game)"
    install -m 644 "$game/assets/fonts/$f" "$tree/fonts/$f"
done
# the fonts must be what the game pins
(cd "$game/assets/fonts" && grep -E ' (NotoSans-Regular.ttf|NotoSansSC-Regular.otf|Tektur-Variable.ttf)$' SHA256SUMS | sha256sum --check --quiet --strict) \
    || die "the fonts do not match the game's SHA256SUMS"
install -m 644 "$here/LICENSE" "$here/THIRD-PARTY.md" "$tree/"

# --- Real-ESRGAN, from the pin ------------------------------------------------------------------------------------
esr_name=""
case "$os-$arch" in
    linux-x86_64) esr_name=realesrgan-ncnn-vulkan ;;
    windows-x86_64) esr_name=realesrgan-ncnn-vulkan-windows ;;
    macos-*) esr_name=realesrgan-ncnn-vulkan-macos ;;
esac
have_esr=0
if [ -n "$esr_name" ]; then
    pin() { awk -v n="$esr_name" -v k="$1" '$1 == "name" { on = ($2 == n) } on && $1 == k { print $2; exit }' "$here/third-party.txt"; }
    url=$(pin url) sha=$(pin sha256)
    [ -n "$url" ] && [ -n "$sha" ] || die "third-party.txt has no pin $esr_name"
    zip="$here/third_party/download/$esr_name.zip"
    if [ ! -f "$zip" ]; then
        mkdir -p "$(dirname "$zip")"
        curl --fail --silent --show-error --location --proto '=https' --tlsv1.2 --output "$zip.part" "$url"
        mv "$zip.part" "$zip"
    fi
    echo "$sha  $zip" | sha256sum --check --quiet --strict || die "$zip does not match third-party.txt"
    ex="$work/esrgan"; mkdir -p "$ex"
    unzip -q "$zip" -d "$ex"
    prog=realesrgan-ncnn-vulkan; [ "$os" != windows ] || prog=realesrgan-ncnn-vulkan.exe
    src=$(find "$ex" -type f -name "$prog" -print -quit | xargs -r dirname)
    [ -n "$src" ] && [ -d "$src/models" ] || die "$zip: no $prog with models/"
    mkdir -p "$tree/realesrgan"
    cp -r "$src/models" "$tree/realesrgan/models"
    install -m 755 "$src/$prog" "$tree/realesrgan/$prog"
    if [ "$os" = windows ]; then
        [ -f "$src/vcomp140.dll" ] || die "$zip: no vcomp140.dll"
        install -m 644 "$src/vcomp140.dll" "$tree/realesrgan/"
    fi
    install -m 644 "$here/packaging/realesrgan-LICENSES.txt" "$tree/realesrgan/LICENSES.txt"
    have_esr=1
fi

# --- README.txt -----------------------------------------------------------------------------------------------------
if [ $have_esr = 1 ]; then
    esr_text="Real-ESRGAN (realesrgan/, licences in realesrgan/LICENSES.txt) is included: it makes the larger textures."
else
    esr_text="Real-ESRGAN is NOT included in this build (no pinned build for $os $arch). The toolbox then enlarges textures
with Lanczos resampling. To get the AI upscaling, put a Real-ESRGAN ncnn Vulkan build (program and models/) into a
realesrgan/ folder beside the program, or on the PATH; the pinned version is in third-party.txt."
fi
case "$os" in
    macos) run_text="The build is not signed. If macOS refuses to open it: right-click the program, Open; or in a terminal
run  xattr -dr com.apple.quarantine <this folder>  once. Run ./$exe" ;;
    windows) run_text="Run $exe (the installer puts it into the Start menu)." ;;
    freebsd) run_text="EXPERIMENTAL build: the window and the platform stack are untested on FreeBSD. Run ./$exe" ;;
    *) run_text="Run ./$exe" ;;
esac
cat >"$tree/README.txt" <<EOT
Drahtseilakt toolbox $version ($os $arch)
=======================================

Prepares levels for the game Drahtseilakt on your machine, from your own copy of Ballance: remastered levels with
rounder shapes and larger textures, and imported community maps. It never goes online and never touches your original
copy. Documentation: https://github.com/eugene-vereshchagin/drahtseilakt-toolbox

$run_text

fonts/   the window's fonts (Tektur, Noto Sans, Noto Sans SC; SIL OFL, licence texts beside them). Keep it beside the
         program: the window reads them from here when it is not run from the game's own folder.
$esr_text

Licence: GPL-3.0-only (LICENSE); what else is in this folder: THIRD-PARTY.md.
EOT

# --- archives -------------------------------------------------------------------------------------------------------
pack_tar() { tar --sort=name --owner=0 --group=0 --numeric-owner --mtime='2026-01-01 00:00Z' -C "$work" -cf - "$folder" | gzip -9n >"$dist/$folder.tar.gz"; }
pack_zip() { (cd "$work" && rm -f "$dist/$folder.zip" && zip -q -r -X -9 "$dist/$folder.zip" "$folder"); }

case "$os" in
    windows)
        pack_zip
        if command -v makensis >/dev/null; then
            makensis -V2 -DSRC="$tree" -DOUT="$dist/$name-$version-$os-$arch-setup.exe" -DVERSION="$version" -DARCH="$arch" "$here/packaging/installer.nsi"
        else
            die "makensis is not installed (apt-get install nsis)"
        fi ;;
    *) pack_tar ;;
esac

# --- .deb and .rpm (Linux): the program, its fonts and Real-ESRGAN in /usr/lib/drahtseilakt-toolbox, a link in /usr/bin. --
# The program finds fonts/ and realesrgan/ beside itself (current_exe is the real file, not the link), so the packages
# need no other package for the fonts (the game's data package is not a dependency: the toolbox reads the game's data
# only through the player's own folders). Vulkan's loader is only recommended: without it Real-ESRGAN cannot run and the
# toolbox falls back to Lanczos.
if [ "$os" = linux ]; then
    lib="/usr/lib/$name" doc="/usr/share/doc/$name"
    stage="$work/stage"
    mkdir -p "$stage$lib" "$stage$doc" "$stage/usr/bin"
    cp -r "$tree/$exe" "$tree/fonts" "$stage$lib/"
    [ $have_esr = 0 ] || cp -r "$tree/realesrgan" "$stage$lib/"
    ln -s "$lib/$exe" "$stage/usr/bin/$name"
    install -m 644 "$here/LICENSE" "$stage$doc/copyright"
    install -m 644 "$here/THIRD-PARTY.md" "$tree/README.txt" "$stage$doc/"
    # fixed modes whatever the build machine's umask
    find "$stage" -type d -exec chmod 755 {} +
    find "$stage" -type f ! -perm /111 -exec chmod 644 {} +
    summary="Prepares Drahtseilakt levels: remastered shapes and larger textures"
    description="Prepares levels for the game Drahtseilakt (the new Ballance engine) on the player's machine, from the player's
own copy of Ballance: rounder shapes, larger textures (Real-ESRGAN when bundled), imported community maps."

    # .deb
    case "$arch" in x86_64) deb_arch=amd64 rpm_arch=x86_64 ;; aarch64) deb_arch=arm64 rpm_arch=aarch64 ;; esac
    mkdir -p "$stage/DEBIAN"
    {
        echo "Package: $name"
        echo "Version: $pkgver"
        echo "Architecture: $deb_arch"
        echo "Maintainer: Veresk <eugene-vereshchagin@users.noreply.github.com>"
        echo "Section: games"
        echo "Priority: optional"
        echo "Homepage: https://github.com/eugene-vereshchagin/drahtseilakt-toolbox"
        echo "Depends: libc6"
        echo "Recommends: libvulkan1, libxkbcommon0, libwayland-client0"
        echo "Description: $summary"
        sed 's/^\(.*\)$/ \1/' <<<"$description"
    } >"$stage/DEBIAN/control"
    dpkg-deb --root-owner-group -Zxz --build "$stage" "$dist/${name}_${pkgver}_${deb_arch}.deb" >/dev/null
    rm -rf "$stage/DEBIAN"

    # .rpm (rpmbuild; the files are staged, so no build step and no stripping: the arch is only a label)
    if command -v rpmbuild >/dev/null; then
        top="$work/rpm"; mkdir -p "$top"/{BUILD,RPMS,SPECS,SRPMS}
        cat >"$top/SPECS/$name.spec" <<EOS
%global debug_package %{nil}
%define _build_id_links none
%define __os_install_post %{nil}
%define _binary_payload w9.gzdio
AutoReqProv: no
Name: $name
Version: $pkgver
Release: 1
Summary: $summary
License: GPL-3.0-only
URL: https://github.com/eugene-vereshchagin/drahtseilakt-toolbox
Requires: glibc
Recommends: vulkan-loader
%description
$description
%install
mkdir -p %{buildroot}
cp -a $stage/usr %{buildroot}/usr
%files
%dir $lib
$lib/*
/usr/bin/$name
%dir $doc
$doc/*
EOS
        rpmbuild -bb --quiet --target "$rpm_arch-linux" --define "_topdir $top" --define "_rpmdir $top/RPMS" \
            --define "_rpmfilename %%{NAME}-%%{VERSION}-%%{RELEASE}.%%{ARCH}.rpm" "$top/SPECS/$name.spec"
        cp "$top"/RPMS/*.rpm "$dist/"
    else
        die "rpmbuild is not installed (apt-get install rpm)"
    fi
fi
ls -l "$dist"
