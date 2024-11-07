# Alarm Modem

Detect an alarm system attempting to dial a phone number, meaning the alarm has been triggered.
This is for older alarm systems that only support landline connections.

Uses FFT to detect DTMF tones in an audio sample. In an ideal world, we'd just listen for the corresponding AT commands,
but for some reason either the modem or alarm doesn't seem to want to do that and I don't want to buy another modem
to figure out which it is.

### Hardware tested on:
- USB Modem: Startech USB56KEMH2
- Alarm panel: Yale HSA6410