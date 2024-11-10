# Alarm Modem

Detect an alarm system attempting to dial a phone number, meaning the alarm has been triggered.
This is for older alarm systems that only support landline connections.

Uses FFT to detect DTMF tones in an audio sample. In an ideal world, we'd just listen for the corresponding AT commands,
but for some reason either the modem or alarm doesn't seem to want to do that and I don't want to buy another modem
to figure out which it is.

Once the tone is detected, a POST request is sent to the webhook URL.
The webhook will be retried every 20 seconds for 6 hours if there is not a successful response from the webhook.

### Env vars

| Key               | Example        | Description                     | Required |
|-------------------|----------------|---------------------------------|----------|
| ALARM_MODEM_PORT  | `/dev/ttyUSB0` | The modem device port.          | Yes      |
| ALARM_MODEM_BAUD  | `9600`         | Modem baud transmit rate.       | No       |
| ALARM_WEBHOOK_URL | `https://...`  | Target webhook URL.             | Yes      |
| ALARM_WEBHOOK_KEY | `token`        | Sent as `Authorization` header. | Yes      |

### Hardware tested on:
- USB Modem: Startech USB56KEMH2
- Alarm panel: Yale HSA6410