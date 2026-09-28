#!/usr/bin/env bash
# Configure the C790's 1080p60 capture pipeline after each Pi reboot.
set -euo pipefail

script_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
edid_file="$script_dir/1080p60edid"
if [[ ! -r "$edid_file" ]]; then
    echo "Missing EDID file: $edid_file" >&2
    exit 1
fi
for command in media-ctl v4l2-ctl timeout; do
    if ! command -v "$command" >/dev/null; then
        echo "Required command not found: $command (install v4l-utils and coreutils)" >&2
        exit 1
    fi
done

# Enumerate existing devices instead of looping forever over absent media nodes.
media_device=""
topology=""
for candidate in /dev/media*; do
    [[ -e "$candidate" ]] || continue
    if ! candidate_topology=$(media-ctl -d "$candidate" -p 2>/dev/null); then
        continue
    fi
    if grep -Eq '^driver[[:space:]]+rp1-cfe$' <<< "$candidate_topology" &&
       grep -q 'tc358743 ' <<< "$candidate_topology"; then
        media_device="$candidate"
        topology="$candidate_topology"
        break
    fi
done
if [[ -z "$media_device" ]]; then
    echo "No rp1-cfe/TC358743 pipeline found. Check the overlay, CAM/DISP1 cable and reboot." >&2
    exit 1
fi

sensor=$(sed -n 's/^- entity [0-9]*: \(tc358743 [^ ]*\) (.*/\1/p' <<< "$topology")
if [[ -z "$sensor" || "$sensor" == *$'\n'* ]]; then
    echo "Expected exactly one TC358743 bridge in $media_device" >&2
    exit 1
fi
subdev=$(media-ctl -d "$media_device" -e "$sensor")
video_device=$(media-ctl -d "$media_device" -e rp1-cfe-csi2_ch0)
echo "Using $media_device, $subdev ($sensor), $video_device"

# Current v4l2-ctl repairs checksums automatically; Debian 13 rejects the old
# separate checksum-fix option.
v4l2-ctl -d "$subdev" --set-edid="file=$edid_file"
sleep 2
if ! timings=$(v4l2-ctl -d "$subdev" --query-dv-timings); then
    echo "No HDMI signal. Activate the C790 display in Windows at 1920x1080 / 60 Hz and rerun." >&2
    exit 1
fi
echo "$timings"
if ! grep -Eq 'Active width:[[:space:]]+1920$' <<< "$timings" ||
   ! grep -Eq 'Active height:[[:space:]]+1080$' <<< "$timings"; then
    echo "This capture setup requires 1920x1080 HDMI input." >&2
    exit 1
fi
v4l2-ctl -d "$subdev" --set-dv-bt-timings query
media-ctl -d "$media_device" -l "'csi2':4 -> 'rp1-cfe-csi2_ch0':0 [1]"
media-ctl -d "$media_device" -V "'csi2':0 [fmt:RGB888_1X24/1920x1080 field:none colorspace:srgb]"
media-ctl -d "$media_device" -V "'csi2':4 [fmt:RGB888_1X24/1920x1080 field:none colorspace:srgb]"
v4l2-ctl -d "$video_device" --set-fmt-video=width=1920,height=1080,pixelformat=RGB3
v4l2-ctl -d "$video_device" --get-fmt-video
timeout 15s v4l2-ctl -d "$video_device" --stream-mmap=3 --stream-skip=30 --stream-count=10 --stream-to=/dev/null
echo "HDMI capture ready. Use capture-test to inspect image content and FISHBOT_SWAP_RB before starting the bot."
