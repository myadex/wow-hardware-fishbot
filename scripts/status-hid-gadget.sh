#!/bin/bash

if [ -d /sys/kernel/config/usb_gadget/pi-hid ]; then
    echo "USB HID gadget is ACTIVE"
    
    # Check if UDC is enabled
    UDC_CONTENT=$(cat /sys/kernel/config/usb_gadget/pi-hid/UDC 2>/dev/null)
    if [ -n "$UDC_CONTENT" ]; then
        echo "  UDC: $UDC_CONTENT"
    else
        echo "  UDC: disabled"
    fi
    
    # Check for device files
    if [ -e /dev/hidg0 ]; then
        echo "  Keyboard device: /dev/hidg0 ✓"
    else
        echo "  Keyboard device: missing ✗"
    fi
    
    if [ -e /dev/hidg1 ]; then
        echo "  Mouse device: /dev/hidg1 ✓"
    else
        echo "  Mouse device: missing ✗"
    fi
else
    echo "USB HID gadget is INACTIVE"
fi
