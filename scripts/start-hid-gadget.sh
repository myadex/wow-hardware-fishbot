#!/bin/bash

# Check if already running
if [ -d /sys/kernel/config/usb_gadget/pi-hid ]; then
    echo "USB HID gadget is already active"
    exit 1
fi

# Create gadget
mkdir -p /sys/kernel/config/usb_gadget/pi-hid
cd /sys/kernel/config/usb_gadget/pi-hid

# Set USB device descriptor
echo 0x1d6b > idVendor  # Linux Foundation
echo 0x0104 > idProduct # Composite Gadget
echo 0x0100 > bcdDevice # v1.0.0
echo 0x0200 > bcdUSB    # USB2

# Set device information
mkdir -p strings/0x409
echo "fedcba9876543210" > strings/0x409/serialnumber
echo "Raspberry Pi Foundation" > strings/0x409/manufacturer
echo "Pi HID Gadget" > strings/0x409/product

# Create configuration
mkdir -p configs/c.1/strings/0x409
echo "Config 1: HID Composite" > configs/c.1/strings/0x409/configuration
echo 250 > configs/c.1/MaxPower

# Create keyboard function
mkdir -p functions/hid.keyboard
echo 1 > functions/hid.keyboard/protocol
echo 1 > functions/hid.keyboard/subclass
echo 8 > functions/hid.keyboard/report_length
echo -ne \\x05\\x01\\x09\\x06\\xa1\\x01\\x05\\x07\\x19\\xe0\\x29\\xe7\\x15\\x00\\x25\\x01\\x75\\x01\\x95\\x08\\x81\\x02\\x95\\x01\\x75\\x08\\x81\\x03\\x95\\x05\\x75\\x01\\x05\\x08\\x19\\x01\\x29\\x05\\x91\\x02\\x95\\x01\\x75\\x03\\x91\\x03\\x95\\x06\\x75\\x08\\x15\\x00\\x25\\x65\\x05\\x07\\x19\\x00\\x29\\x65\\x81\\x00\\xc0 > functions/hid.keyboard/report_desc

# Create mouse function
mkdir -p functions/hid.mouse
echo 2 > functions/hid.mouse/protocol
echo 1 > functions/hid.mouse/subclass
echo 4 > functions/hid.mouse/report_length
echo -ne \\x05\\x01\\x09\\x02\\xa1\\x01\\x09\\x01\\xa1\\x00\\x05\\x09\\x19\\x01\\x29\\x03\\x15\\x00\\x25\\x01\\x95\\x03\\x75\\x01\\x81\\x02\\x95\\x01\\x75\\x05\\x81\\x03\\x05\\x01\\x09\\x30\\x09\\x31\\x09\\x38\\x15\\x81\\x25\\x7f\\x75\\x08\\x95\\x03\\x81\\x06\\xc0\\xc0 > functions/hid.mouse/report_desc

# Link functions to configuration
ln -s functions/hid.keyboard configs/c.1/
ln -s functions/hid.mouse configs/c.1/

# Enable gadget
ls /sys/class/udc > UDC

echo "USB HID gadget started successfully"
echo "Keyboard device: /dev/hidg0"
echo "Mouse device: /dev/hidg1"
