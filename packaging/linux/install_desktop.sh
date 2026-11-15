#!/usr/bin/env bash
set -euo pipefail

# Run this copy from dist/linux so the launcher targets that exact distribution.
package_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
app_id=io.github.TheDoctorTTV.AssetExtractor
binary_path="$package_dir/asset-extractor"
icon_path="$package_dir/$app_id.png"
if [[ ! -x "$binary_path" || ! -f "$icon_path" ]]; then
    echo "Run install_desktop.sh from a complete dist/linux package after building." >&2
    exit 1
fi
if [[ "$binary_path" == *'='* || "$binary_path" == *$'\n'* || "$binary_path" == *$'\r'* ]]; then
    echo "The package path contains a character unsupported by desktop launchers. Move it and run this script again." >&2
    exit 1
fi
if [[ "${XDG_DATA_HOME:-}" == /* ]]; then
    desktop_data_dir="$XDG_DATA_HOME"
else
    desktop_data_dir="$HOME/.local/share"
fi
applications_dir="$desktop_data_dir/applications"
icon_root="$desktop_data_dir/icons/hicolor"
mkdir -p -- "$applications_dir"
install -D -m 644 -- "$icon_path" "$icon_root/256x256/apps/$app_id.png"

# Exec values have two escaping layers: desktop-entry strings and quoted arguments.
# Do not use shell quoting; desktop launchers do not interpret it as a shell command.
desktop_exec_value='"'
for ((character_index = 0; character_index < ${#binary_path}; character_index++)); do
    character="${binary_path:character_index:1}"
    case "$character" in
        '\') desktop_exec_value+='\\\\' ;;
        '"'|'$'|'`') desktop_exec_value+='\\'; desktop_exec_value+="$character" ;;
        '%') desktop_exec_value+='%%' ;;
        *) desktop_exec_value+="$character" ;;
    esac
done
desktop_exec_value+='"'

desktop_temp="$(mktemp "$applications_dir/.$app_id.XXXXXX")"
trap 'rm -f -- "$desktop_temp"' EXIT
cat > "$desktop_temp" <<EOF
[Desktop Entry]
Type=Application
Name=Asset Extractor
Comment=Extract Beat Saber bundle assets and map data
Exec=$desktop_exec_value
Icon=$app_id
Terminal=false
Categories=Utility;
StartupNotify=true
StartupWMClass=$app_id
EOF
chmod 644 -- "$desktop_temp"
mv -f -- "$desktop_temp" "$applications_dir/$app_id.desktop"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$applications_dir" >/dev/null 2>&1 || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$icon_root" >/dev/null 2>&1 || true
fi
echo "Registered Linux launcher and icon: $applications_dir/$app_id.desktop"
