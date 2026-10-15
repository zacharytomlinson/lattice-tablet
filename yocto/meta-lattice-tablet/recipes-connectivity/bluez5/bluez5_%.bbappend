# BLE discovery/advertising for room-localization measurements. Keep the
# diagnostic tools without pulling in audio, file transfer, or network profiles.
PACKAGECONFIG = "readline tools systemd udev"
NOINST_TOOLS = "tools/btmgmt"
