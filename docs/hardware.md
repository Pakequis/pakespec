# LTDZ 35-4400M hardware

Sources: a photo of the board (2019 blog post), the manufacturer's schematic and instructions (not redistributed), and articles from Silicon Chip and Practical Electronics.

## Identification of the owner's unit

Silkscreen: "35M-4400M Heterodyne Spectrum Analyzer, Onboard Tracking Source", LTDZ brand, date 2019-03-08, "RBW: 5k-120k", "RF IN recommend RFin <= +5dB". Shipped by Banggood for review in 2019. The hardware version (for example V5.0) is not written in the photo.

## Chips (manufacturer's schematic, 2019-03-09)

- MCU: STM32F103C8T6, 8 MHz crystal.
- USB serial: CH340G, 12 MHz crystal.
- Two ADF4351: one is the analyzer's local oscillator (L.O.), the other is the tracking source (T.G.). 25 MHz reference.
- IAM-81008 mixer (signal input through SMA), band-pass filter, AD8307 logarithmic detector read by the STM32 ADC.
- 5 V supply from USB, two AMS1117 regulators (3.3 V) and a PS1302 (4.85 V for the mixer). Current draw: ~100 mA idle and ~350 mA sweeping with the tracking source.

## Connectors and controls

- Micro-USB (CH340G).
- SMA RF IN and SMA RF OUT ("Tracking source").
- Button S1 ("PRESS THE KEY TO OPEN Tracking source"): turns the tracking source on.
- LEDs: tracking active, synthesizers locked (lock), power, analyzer running.
- **P1, 4 pins at the top**: the silkscreen says "USART", but in the schematic it is the STM32 **SWD** port (SWC and SWD). In theory it allows flashing and reading the firmware with an ST-Link, if readout protection is not enabled. Check the exact pinout on the board before connecting anything.

## Manufacturer specifications

- Analyzer: 35 to 4400 MHz, RBW 5 to 120 kHz, more than 800 points per second, logarithmic dynamic range above 50 dB, input up to 10 dBm (the board recommends up to +5 dBm).
- Sweep step: 125 Hz to 8 kHz depending on the frequency range (manufacturer data; not checked).
- Tracking: 120 kHz away from the local oscillator, used to measure S12 and S21; antennas need a reflection bridge built separately. Output of about 0 dBm according to an external source (not measured).

## Performance according to external sources

Noise floor around −76 dBm; no input amplifier (blind below ~50 µV); fixed, wide RBW (narrow signals show up with a valley between two peaks, wrong amplitudes); spurious peaks at ±25 and ±75 MHz from a 2.5 GHz signal; ripples near 3.7 and 4.1 GHz that may come from the cable. Tip: use a short SMA cable in tracking tests and check odd readings with another analyzer.
