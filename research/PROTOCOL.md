# Midi Fighter Twister configuration protocol

Recovered from **Midi Fighter Utility** (macOS DMG from djtechtools.com, universal Mach-O,
arm64 slice SHA-256 `932d83af…f432`) using REA (JavaScript analysis + Mach-O inspection) and
Apple `objdump`. No native decompiler (Ghidra/Hopper/IDA) was installed, so the native
findings come from reading arm64 disassembly of the named C++ symbols.

## Where things live in the Utility

- `Contents/Resources/plugins/device.mf_twister.js`: the device plugin. It defines every
  setting (tag number, label, range, options, defaults) and the per-encoder bulk transfer
  loop. Hosted by the Qt app through a `CONFIG` object.
- `Contents/MacOS/Midi Fighter Utility`: the C++ host (`ConfigWidget`, `MidiSerializer`, `Device`,
  `Midi`). It does the SysEx framing.

## Framing

Every message: `F0 00 01 79 <payload> F7`. `00 01 79` is the DJTT manufacturer id
(`MidiSerializer` keeps it as the u16 `0x0179`). Every payload byte is masked `& 0x7F`.
Messages go out through `Midi::send(MidiCommand 0xF, …)`.

| Purpose                 | Payload                                         | Source (arm64 address)                         |
|-------------------------|-------------------------------------------------|------------------------------------------------|
| Request global config   | `02 00`                                         | `Device::requestConfig` @ `0x10012b48c`        |
| Global config reply     | `02 01 (tag value)*`                            | `ConfigWidget::sysEx` @ `0x100121974`          |
| Write global config     | `01 (tag value)*` (serializer contents)         | `MidiSerializer::send(midi, 1)` @ `0x10011ee9c`, called from `ConfigWidget::sendToDevice` |
| Request encoder record  | `04 01 <record>`; waits up to 2500 ms           | `ConfigWidget::pullBulkTransfer` @ `0x100126f8c` |
| Encoder record chunk    | `04 00 <record> <part> <total> <len> <data…>`   | `ConfigWidget::pushBulkTransfer` @ `0x100126728`, parsed in `ConfigWidget::sysEx` |

- `record` = `bank * 16 + encoder + 1` (1-based). A 4-bank firmware answers records 1–64 and an
  8-bank firmware answers 1–128. The JS keeps pulling until 16 records in a row fail.
- Bulk data is split into chunks of at most **24 bytes**. `part` is 1-based, `total` =
  `ceil(len / 24)`, and `len` is the chunk's byte count. The receiver appends `data` until
  `part == total`.
- Writes go in this order: every encoder record (bulk), then the global config (`01 …`).

## Encoder record (16 tag/value pairs = 32 bytes)

| Tag | Setting              | Values |
|-----|----------------------|--------|
| 10  | Center Detent        | 0/1 |
| 11  | Sensitivity          | 0 360°, 1 Responsive, 2 Velocity Sensitive |
| 12  | Switch Action        | 0 CC Hold, 1 CC Toggle, 2 Note Hold, 3 Note Toggle, 4 Reset (0), 5 Reset (127), 6 Fine Adjust, 7 Shift (Hold), 8 Shift (Toggle), 9 Re-send Program, 10 Disabled, 11 MCU Button |
| 13  | Switch MIDI Channel  | 1–16 |
| 14  | Switch MIDI Number   | 0–127 |
| 16  | Encoder MIDI Channel | 1–16 |
| 17  | Encoder MIDI Number  | 0–127 |
| 18  | Encoder Type         | 0 Note, 1 CC, 2 Relative 3F/41, 3 Button Velocity, 4 Mouse Move, 5 Mouse Scroll, 6 Program Change, 7 MCU Fader, 8 MCU V-Pot, 9 Disabled |
| 19  | Active Color         | palette index 0–127 |
| 20  | Inactive Color       | palette index 0–127 |
| 21  | Detent Color         | 0–127 (red→blue blend) |
| 22  | Indicator            | 0 Dot, 1 Bar, 2 Blended Bar, 3 Spread |
| 23  | Super Knob           | 0/1 |
| 24  | Shift MIDI Channel   | 1–16 |

Tag 15 and any other tags are kept as-is (shown raw in the app).

## Global settings

| Tag | Setting | Values |
|-----|---------|--------|
| 0   | System Channel | 1–16 (default 4) |
| 1   | Bank Side Buttons | 0/1 |
| 2–7 | Side buttons L-top, L-mid, L-bot, R-top, R-mid, R-bot | action index (20 options on fw ≥ 2023-12-11, 13 on older) |
| 8, 9 | Super Knob Start / End | 0–127 |
| 31  | RGB Brightness | 0–127 |
| 32  | Indicator Brightness | 0–127 |
| 33  | Color Map | 0 Classic, 1 Expanded |
| 34, 35 | Encoder / Button animation channel | 1–16 |
| 36  | Sleep Timer | 0 Off … 7 60 min |
| 37  | Sleep Animation | 0 LEDs off, 1 Rainbow |
| 38  | Bank Change Animations | 0/1 |
| 39, 40 | Detent Size / Dead Zone Size | size + 1 (0 = not sent) |
| 41  | Disable Shift Switch MIDI | 1 off, 2 on (0 = not sent) |

## Not verified

- These findings come from static analysis only. None of the traffic was captured from a real device.
- The disassembly doesn't show whether the device acknowledges writes or needs a delay
  between messages. The app leaves 3 ms between messages.
