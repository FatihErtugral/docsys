# UART DMA timing, second board

Same numbers as yesterday, confirmed on the second board.

| setting | value |
|---|---|
| channel | DMA1_CH5 |
| baud | 115200 |
| frame gap | 3.2 ms |
| buffer | 512 bytes |

512 bytes is the smallest ring that survived an hour without an overrun.
