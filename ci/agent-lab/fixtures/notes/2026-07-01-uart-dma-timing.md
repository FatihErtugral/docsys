# UART DMA timing on the F4 board

Measured on the bench with the logic analyzer, application firmware.

| setting | value |
|---|---|
| channel | DMA1_CH5 |
| baud | 115200 |
| frame gap | 3.2 ms |

The gap is what the receiver needs before the next frame starts; below it
the ring buffer overruns and the checksum fails on every third frame.
