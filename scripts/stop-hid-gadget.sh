#!/bin/bash

if [ ! -d /sys/kernel/config/usb_gadget/pi-hid ]; then
    echo "USB HID gadget is not active"
    exit 1
fi

cd /sys/kernel/config/usb_gadget/pi-hid

# Step 1: Disable the gadget first
echo "" > UDC

# Step 2: Remove symlinks from configuration
rm -f configs/c.1/hid.keyboard
rm -f configs/c.1/hid.mouse

# Step 3: Remove configuration strings
rmdir configs/c.1/strings/0x409 2>/dev/null

# Step 4: Remove configuration
rmdir configs/c.1 2>/dev/null

# Step 5: Remove functions
rmdir functions/hid.keyboard 2>/dev/null
rmdir functions/hid.mouse 2>/dev/null

# Step 6: Remove device strings
rmdir strings/0x409 2>/dev/null

# Step 7: Go back and remove the gadget directory
cd ..
rmdir pi-hid 2>/dev/null

echo "USB HID gadget stopped"
