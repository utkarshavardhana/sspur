# Token benchmark results

Tokenizers: o200k, cl100k. Ratio = tokens / Python tokens (lower is better).

## 1_word_freq

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 3 | 54 | 0.83 | 54 | 0.83 |
| python | 7 | 65 | 1.00 | 65 | 1.00 |
| typescript | 9 | 114 | 1.75 | 112 | 1.72 |
| go | 35 | 203 | 3.12 | 201 | 3.09 |
| rust | 16 | 162 | 2.49 | 157 | 2.42 |
| cpp | 26 | 221 | 3.40 | 221 | 3.40 |

## 2_order_logic

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 13 | 159 | 0.62 | 159 | 0.62 |
| python | 51 | 256 | 1.00 | 258 | 1.00 |
| typescript | 30 | 302 | 1.18 | 302 | 1.17 |
| go | 61 | 338 | 1.32 | 338 | 1.31 |
| rust | 55 | 348 | 1.36 | 345 | 1.34 |
| cpp | 46 | 360 | 1.41 | 359 | 1.39 |

## 3_retry_fetch

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 11 | 142 | 0.81 | 142 | 0.81 |
| python | 30 | 176 | 1.00 | 176 | 1.00 |
| typescript | 22 | 197 | 1.12 | 197 | 1.12 |
| go | 46 | 283 | 1.61 | 277 | 1.57 |
| rust | 31 | 255 | 1.45 | 255 | 1.45 |
| cpp | 32 | 300 | 1.70 | 298 | 1.69 |

## 4_parallel_combine

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 6 | 83 | 0.61 | 82 | 0.60 |
| python | 22 | 135 | 1.00 | 136 | 1.00 |
| typescript | 10 | 102 | 0.76 | 96 | 0.71 |
| go | 35 | 202 | 1.50 | 197 | 1.45 |
| rust | 16 | 126 | 0.93 | 127 | 0.93 |
| cpp | 19 | 175 | 1.30 | 174 | 1.28 |

## 5_crud_service_infra

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 12 | 138 | 0.30 | 137 | 0.30 |
| python | 47 | 459 | 1.00 | 456 | 1.00 |
| typescript | 46 | 577 | 1.26 | 566 | 1.24 |

## 6_ring_buffer

| Lang | Lines | o200k | o200k ratio | cl100k | cl100k ratio |
|---|---|---|---|---|---|
| sspur | 18 | 214 | 0.98 | 214 | 0.98 |
| python | 29 | 218 | 1.00 | 219 | 1.00 |
| typescript | 29 | 174 | 0.80 | 174 | 0.79 |
| go | 34 | 188 | 0.86 | 188 | 0.86 |
| rust | 42 | 281 | 1.29 | 280 | 1.28 |
| cpp | 39 | 280 | 1.28 | 282 | 1.29 |

## Summary (geometric mean of ratio vs Python)

| Lang | o200k | cl100k | Median o200k ratio |
|---|---|---|---|
| sspur | 0.65 | 0.65 | 0.71 |
| python | 1.00 | 1.00 | 1.00 |
| typescript | 1.10 | 1.08 | 1.15 |
| go | 1.54 | 1.51 | 1.50 |
| rust | 1.43 | 1.41 | 1.36 |
| cpp | 1.68 | 1.68 | 1.41 |
