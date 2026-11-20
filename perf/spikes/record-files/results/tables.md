### Corpus

| Size | Layout | Records | Files | File bytes | Ledger bytes | Index bytes | Build s |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 4,396 | 1 | 1.0 MiB | 1.6 MiB | 164 KiB | 0.1 |
| 1MiB | A2 | 4,396 | 1 | 1.0 MiB | 1.6 MiB | 164 KiB | 0.1 |
| 1MiB | B | 3,498 | 1 | 1.0 MiB | 1.3 MiB | 132 KiB | 0.1 |
| 1MiB | C | 4,396 | 1 | 1.0 MiB | 1.6 MiB | 172 KiB | 0.1 |
| 16MiB | A | 68,436 | 1 | 16.0 MiB | 25.0 MiB | 2.4 MiB | 0.5 |
| 16MiB | A2 | 68,436 | 1 | 16.0 MiB | 25.0 MiB | 2.4 MiB | 0.5 |
| 16MiB | B | 54,630 | 1 | 16.0 MiB | 20.0 MiB | 1.9 MiB | 0.4 |
| 16MiB | C | 68,434 | 4 | 16.0 MiB | 25.2 MiB | 2.5 MiB | 0.5 |
| 64MiB | A | 275,516 | 1 | 64.0 MiB | 100.8 MiB | 9.6 MiB | 1.7 |
| 64MiB | A2 | 275,516 | 1 | 64.0 MiB | 100.8 MiB | 9.6 MiB | 1.7 |
| 64MiB | B | 220,658 | 1 | 64.0 MiB | 80.9 MiB | 7.7 MiB | 1.4 |
| 64MiB | C | 275,511 | 16 | 64.0 MiB | 101.5 MiB | 10.2 MiB | 1.8 |
| 256MiB | A | 1,103,069 | 1 | 256.0 MiB | 403.7 MiB | 38.6 MiB | 7.5 |
| 256MiB | A2 | 1,103,069 | 1 | 256.0 MiB | 403.7 MiB | 38.6 MiB | 7.7 |
| 256MiB | B | 882,908 | 1 | 256.0 MiB | 324.0 MiB | 30.9 MiB | 6.1 |
| 256MiB | C | 1,103,051 | 61 | 256.0 MiB | 406.6 MiB | 40.7 MiB | 8.0 |

### append

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 13.539 | 15.541 | 20.648 | 27.359 | 7.062 | 6.733 | 0.000 | 36 KiB | 44 KiB | 2.0 | - | 0 |
| 1MiB | B | 200 | 13.278 | 17.225 | 21.762 | 36.231 | 7.057 | 6.738 | 0.000 | 35 KiB | 44 KiB | 2.0 | 0.0% | 0 |
| 1MiB | C | 200 | 13.800 | 15.986 | 20.295 | 36.909 | 7.103 | 6.773 | 0.000 | 36 KiB | 44 KiB | 2.0 | - | 0 |
| 16MiB | A | 200 | 13.283 | 16.097 | 24.804 | 44.634 | 7.087 | 6.751 | 0.000 | 49 KiB | 58 KiB | 2.0 | - | 0 |
| 16MiB | B | 200 | 13.404 | 18.005 | 25.460 | 27.192 | 7.205 | 6.851 | 0.000 | 48 KiB | 56 KiB | 2.0 | 0.0% | 0 |
| 16MiB | C | 200 | 13.266 | 18.624 | 24.500 | 35.141 | 7.190 | 6.765 | 0.000 | 49 KiB | 58 KiB | 2.0 | - | 0 |
| 64MiB | A | 200 | 13.682 | 20.535 | 30.651 | 37.135 | 7.637 | 7.065 | 0.000 | 64 KiB | 73 KiB | 2.0 | - | 0 |
| 64MiB | B | 200 | 13.184 | 18.826 | 24.739 | 35.942 | 7.286 | 6.798 | 0.000 | 63 KiB | 72 KiB | 2.0 | 0.0% | 0 |
| 64MiB | C | 200 | 13.803 | 19.152 | 23.666 | 35.918 | 7.537 | 7.091 | 0.000 | 64 KiB | 73 KiB | 2.0 | - | 0 |
| 256MiB | A | 200 | 13.046 | 15.439 | 24.141 | 36.732 | 7.070 | 6.504 | 0.000 | 70 KiB | 78 KiB | 2.0 | - | 0 |
| 256MiB | B | 200 | 13.345 | 19.850 | 27.610 | 36.875 | 7.437 | 7.040 | 0.000 | 69 KiB | 78 KiB | 2.0 | 0.0% | 0 |
| 256MiB | C | 200 | 13.425 | 20.155 | 28.979 | 35.860 | 7.559 | 7.074 | 0.000 | 69 KiB | 78 KiB | 2.0 | - | 0 |

### read_last_50

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 0.019 | 0.023 | 0.044 | 0.086 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 1MiB | B | 200 | 0.020 | 0.031 | 0.046 | 0.079 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | 0.0% | 0 |
| 1MiB | C | 200 | 0.020 | 0.022 | 0.035 | 0.069 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 16MiB | A | 200 | 0.019 | 0.024 | 0.031 | 0.075 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 16MiB | B | 200 | 0.020 | 0.023 | 0.032 | 0.070 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | 0.0% | 0 |
| 16MiB | C | 200 | 0.020 | 0.028 | 0.042 | 0.077 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 64MiB | A | 200 | 0.020 | 0.032 | 0.046 | 0.092 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 64MiB | B | 200 | 0.020 | 0.026 | 0.034 | 0.068 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | 0.0% | 0 |
| 64MiB | C | 200 | 0.020 | 0.037 | 0.061 | 0.089 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 256MiB | A | 200 | 0.020 | 0.024 | 0.036 | 0.081 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |
| 256MiB | B | 200 | 0.020 | 0.031 | 0.037 | 0.071 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | 0.0% | 0 |
| 256MiB | C | 200 | 0.020 | 0.029 | 0.050 | 0.068 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 1.0 | - | 0 |

### unread_after_p

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 0.074 | 0.111 | 0.125 | 0.184 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 1MiB | B | 200 | 0.079 | 0.116 | 0.141 | 0.159 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | 0.0% | 0 |
| 1MiB | C | 200 | 0.078 | 0.115 | 0.126 | 0.165 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 16MiB | A | 200 | 0.074 | 0.118 | 0.126 | 0.142 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 16MiB | B | 200 | 0.075 | 0.116 | 0.124 | 0.131 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | 0.0% | 0 |
| 16MiB | C | 200 | 0.076 | 0.121 | 0.141 | 0.159 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 64MiB | A | 200 | 0.076 | 0.124 | 0.139 | 0.179 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 64MiB | B | 200 | 0.074 | 0.118 | 0.132 | 0.150 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | 0.0% | 0 |
| 64MiB | C | 200 | 0.077 | 0.121 | 0.137 | 0.159 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 256MiB | A | 200 | 0.074 | 0.115 | 0.123 | 0.158 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |
| 256MiB | B | 200 | 0.075 | 0.116 | 0.122 | 0.172 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | 0.0% | 0 |
| 256MiB | C | 200 | 0.074 | 0.116 | 0.142 | 0.154 | 0.000 | 0.000 | 0.000 | 0 KiB | 0 KiB | 2.0 | - | 0 |

### edit_recent

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 13.955 | 19.552 | 23.351 | 33.882 | 7.834 | 7.259 | 0.000 | 119 KiB | 127 KiB | 3.0 | - | 0 |
| 1MiB | A2 | 200 | 32.580 | 38.083 | 45.643 | 53.566 | 7.671 | 25.490 | 0.000 | 1.1 MiB | 1.1 MiB | 3.0 | - | 0 |
| 1MiB | B | 200 | 9.604 | 14.312 | 17.131 | 21.128 | 6.733 | 3.961 | 0.000 | 48 KiB | 56 KiB | 3.0 | 22.0% | 0 |
| 1MiB | C | 200 | 14.094 | 18.949 | 22.835 | 56.716 | 7.570 | 7.111 | 0.000 | 124 KiB | 132 KiB | 3.0 | - | 0 |
| 16MiB | A | 200 | 13.809 | 19.598 | 23.147 | 26.567 | 7.548 | 6.821 | 0.000 | 134 KiB | 143 KiB | 3.0 | - | 0 |
| 16MiB | A2 | 200 | 224.063 | 408.776 | 690.018 | 823.035 | 8.885 | 237.928 | 0.000 | 16.1 MiB | 16.1 MiB | 3.0 | - | 0 |
| 16MiB | B | 200 | 9.949 | 15.796 | 22.554 | 33.927 | 7.316 | 4.210 | 0.000 | 56 KiB | 64 KiB | 3.0 | 22.0% | 0 |
| 16MiB | C | 200 | 13.739 | 18.068 | 23.923 | 26.523 | 7.493 | 6.870 | 0.000 | 129 KiB | 137 KiB | 3.0 | - | 0 |
| 64MiB | A | 200 | 14.136 | 17.918 | 24.959 | 25.556 | 7.665 | 7.064 | 0.000 | 138 KiB | 146 KiB | 3.0 | - | 0 |
| 64MiB | A2 | 200 | 782.653 | 868.765 | 981.575 | 1438.137 | 8.396 | 773.322 | 0.000 | 64.1 MiB | 64.1 MiB | 3.0 | - | 0 |
| 64MiB | B | 200 | 10.262 | 15.090 | 16.040 | 18.222 | 7.172 | 4.124 | 0.000 | 50 KiB | 58 KiB | 3.0 | 19.5% | 0 |
| 64MiB | C | 200 | 13.986 | 15.823 | 42.285 | 47.024 | 7.410 | 6.942 | 0.000 | 121 KiB | 130 KiB | 3.0 | - | 0 |
| 256MiB | A | 200 | 14.093 | 18.671 | 23.106 | 33.320 | 7.773 | 7.053 | 0.000 | 136 KiB | 145 KiB | 3.0 | - | 0 |
| 256MiB | A2 | 200 | 3336.964 | 4040.067 | 6087.046 | 7560.370 | 9.231 | 3443.778 | 0.000 | 256.1 MiB | 256.1 MiB | 3.0 | - | 0 |
| 256MiB | B | 200 | 9.756 | 14.034 | 18.018 | 20.746 | 6.766 | 3.886 | 0.000 | 47 KiB | 55 KiB | 3.0 | 17.0% | 0 |
| 256MiB | C | 200 | 13.840 | 17.634 | 23.357 | 24.158 | 7.492 | 6.941 | 0.000 | 124 KiB | 132 KiB | 3.0 | - | 0 |

### edit_random

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 19.513 | 29.046 | 33.062 | 35.383 | 11.548 | 8.678 | 0.000 | 1.0 MiB | 1.0 MiB | 3.0 | - | 0 |
| 1MiB | A2 | 200 | 36.644 | 50.223 | 57.390 | 76.817 | 11.993 | 26.355 | 0.000 | 1.5 MiB | 1.5 MiB | 3.0 | - | 0 |
| 1MiB | B | 200 | 9.798 | 20.542 | 23.880 | 29.325 | 7.667 | 4.552 | 0.000 | 279 KiB | 288 KiB | 3.0 | 27.0% | 0 |
| 1MiB | C | 200 | 19.147 | 28.948 | 33.087 | 33.618 | 11.334 | 8.244 | 0.000 | 1.0 MiB | 1.0 MiB | 3.0 | - | 0 |
| 16MiB | A | 200 | 161.541 | 270.619 | 292.220 | 304.603 | 85.704 | 71.541 | 0.000 | 18.8 MiB | 18.8 MiB | 3.0 | - | 0 |
| 16MiB | A2 | 200 | 296.591 | 365.689 | 377.261 | 410.921 | 84.346 | 212.834 | 0.000 | 25.7 MiB | 25.7 MiB | 3.0 | - | 0 |
| 16MiB | B | 200 | 10.232 | 189.661 | 243.610 | 252.887 | 17.017 | 13.843 | 0.000 | 2.6 MiB | 2.6 MiB | 3.0 | 16.5% | 0 |
| 16MiB | C | 200 | 50.006 | 90.795 | 100.311 | 111.500 | 28.887 | 22.101 | 0.000 | 4.8 MiB | 4.8 MiB | 3.0 | - | 0 |
| 64MiB | A | 200 | 588.704 | 1058.169 | 1115.897 | 1149.504 | 307.877 | 278.165 | 0.000 | 78.5 MiB | 78.7 MiB | 3.0 | - | 0 |
| 64MiB | A2 | 200 | 1113.520 | 1369.507 | 1469.971 | 1480.355 | 304.077 | 801.380 | 0.000 | 107.8 MiB | 107.8 MiB | 3.0 | - | 0 |
| 64MiB | B | 200 | 11.692 | 693.832 | 896.328 | 992.591 | 58.867 | 56.102 | 0.000 | 15.2 MiB | 15.2 MiB | 3.0 | 22.5% | 0 |
| 64MiB | C | 200 | 47.549 | 94.289 | 102.737 | 108.458 | 29.213 | 22.075 | 0.000 | 5.2 MiB | 5.2 MiB | 3.0 | - | 0 |
| 256MiB | A | 200 | 2014.611 | 4052.977 | 4244.045 | 4439.608 | 1073.400 | 982.289 | 0.000 | 301.2 MiB | 301.7 MiB | 3.0 | - | 0 |
| 256MiB | A2 | 200 | 4494.586 | 5930.177 | 6675.842 | 9161.230 | 1165.535 | 3394.217 | 0.000 | 431.1 MiB | 431.1 MiB | 3.0 | - | 0 |
| 256MiB | B | 200 | 10.554 | 1818.541 | 3107.330 | 3898.453 | 130.413 | 132.639 | 0.000 | 36.7 MiB | 36.8 MiB | 3.0 | 17.5% | 0 |
| 256MiB | C | 200 | 54.108 | 107.476 | 142.404 | 159.727 | 32.308 | 26.162 | 0.000 | 5.0 MiB | 5.1 MiB | 3.0 | - | 0 |

### redact_recent

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 26.828 | 31.598 | 35.823 | 41.879 | 13.789 | 6.209 | 6.992 | 168 KiB | 180 KiB | 3.0 | - | 0 |
| 1MiB | B | 200 | 23.395 | 28.364 | 30.490 | 66.977 | 13.893 | 3.336 | 6.993 | 60 KiB | 72 KiB | 3.0 | 0.0% | 0 |
| 1MiB | C | 200 | 27.844 | 32.270 | 38.826 | 39.385 | 14.303 | 6.579 | 7.298 | 177 KiB | 189 KiB | 3.0 | - | 0 |
| 16MiB | A | 200 | 28.081 | 34.623 | 39.819 | 49.354 | 14.625 | 6.454 | 7.518 | 208 KiB | 220 KiB | 3.0 | - | 0 |
| 16MiB | B | 200 | 23.751 | 28.525 | 34.769 | 36.078 | 14.028 | 3.454 | 7.109 | 60 KiB | 72 KiB | 3.0 | 0.0% | 0 |
| 16MiB | C | 200 | 27.672 | 32.261 | 39.266 | 45.352 | 14.519 | 6.258 | 7.283 | 160 KiB | 173 KiB | 3.0 | - | 0 |
| 64MiB | A | 200 | 33.327 | 44.893 | 76.920 | 86.246 | 18.077 | 8.302 | 9.069 | 187 KiB | 200 KiB | 3.0 | - | 0 |
| 64MiB | B | 200 | 23.518 | 26.659 | 34.897 | 43.892 | 13.831 | 3.379 | 6.944 | 57 KiB | 68 KiB | 3.0 | 0.0% | 0 |
| 64MiB | C | 200 | 27.104 | 33.768 | 47.286 | 48.670 | 14.218 | 6.410 | 7.171 | 155 KiB | 167 KiB | 3.0 | - | 0 |
| 256MiB | A | 200 | 27.365 | 36.948 | 51.085 | 88.021 | 14.441 | 6.441 | 7.337 | 178 KiB | 190 KiB | 3.0 | - | 0 |
| 256MiB | B | 200 | 23.855 | 31.013 | 56.480 | 72.786 | 14.573 | 3.482 | 7.338 | 56 KiB | 68 KiB | 3.0 | 0.0% | 0 |
| 256MiB | C | 200 | 27.882 | 33.608 | 45.331 | 76.626 | 14.536 | 6.579 | 7.497 | 176 KiB | 188 KiB | 3.0 | - | 0 |

### redact_random

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 200 | 33.423 | 40.502 | 46.193 | 52.305 | 17.301 | 7.834 | 8.138 | 1.2 MiB | 1.2 MiB | 3.0 | - | 0 |
| 1MiB | B | 200 | 23.050 | 25.997 | 31.572 | 37.001 | 13.476 | 3.290 | 6.802 | 54 KiB | 66 KiB | 3.0 | 0.0% | 0 |
| 1MiB | C | 200 | 33.907 | 40.881 | 46.749 | 58.527 | 17.513 | 7.917 | 8.282 | 1.2 MiB | 1.3 MiB | 3.0 | - | 0 |
| 16MiB | A | 200 | 164.821 | 287.314 | 295.884 | 302.566 | 83.433 | 70.526 | 9.844 | 18.0 MiB | 18.0 MiB | 3.0 | - | 0 |
| 16MiB | B | 200 | 23.555 | 27.367 | 31.937 | 46.395 | 13.885 | 3.522 | 6.929 | 56 KiB | 67 KiB | 3.0 | 0.0% | 0 |
| 16MiB | C | 200 | 67.250 | 107.973 | 114.845 | 118.185 | 30.359 | 24.432 | 13.681 | 4.5 MiB | 4.5 MiB | 3.0 | - | 0 |
| 64MiB | A | 200 | 569.073 | 1230.934 | 1727.645 | 1810.982 | 311.075 | 285.482 | 4.031 | 72.8 MiB | 73.0 MiB | 3.0 | - | 0 |
| 64MiB | B | 200 | 23.974 | 28.666 | 32.733 | 50.313 | 14.002 | 3.557 | 7.139 | 65 KiB | 77 KiB | 3.0 | 0.0% | 0 |
| 64MiB | C | 200 | 73.044 | 109.880 | 126.006 | 132.324 | 30.905 | 25.283 | 13.652 | 4.9 MiB | 4.9 MiB | 3.0 | - | 0 |
| 256MiB | A | 200 | 2087.023 | 4208.154 | 5174.112 | 9203.370 | 1157.655 | 1076.478 | 6.709 | 310.1 MiB | 311.5 MiB | 3.0 | - | 0 |
| 256MiB | B | 200 | 24.019 | 27.601 | 36.587 | 52.373 | 14.109 | 3.511 | 7.135 | 83 KiB | 94 KiB | 3.0 | 0.0% | 0 |
| 256MiB | C | 200 | 78.971 | 121.621 | 135.806 | 153.346 | 33.686 | 30.106 | 15.083 | 5.3 MiB | 5.3 MiB | 3.0 | - | 0 |

### reanchor_typo

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 20 | 14.239 | 18.372 | 26.350 | 26.350 | 15.129 | 0.000 | 0.000 | 18 KiB | 22 KiB | 0.0 | - | 1 |
| 1MiB | B | 20 | 13.779 | 15.457 | 22.938 | 22.938 | 14.350 | 0.000 | 0.000 | 17 KiB | 21 KiB | 0.0 | 0.0% | 1 |
| 1MiB | C | 20 | 14.478 | 15.654 | 29.472 | 29.472 | 15.325 | 0.000 | 0.000 | 19 KiB | 23 KiB | 0.0 | - | 1 |
| 16MiB | A | 20 | 128.707 | 140.698 | 168.791 | 168.791 | 130.536 | 0.000 | 0.000 | 17 KiB | 21 KiB | 0.0 | - | 1 |
| 16MiB | B | 20 | 108.934 | 118.652 | 120.493 | 120.493 | 108.989 | 0.000 | 0.000 | 16 KiB | 20 KiB | 0.0 | 0.0% | 1 |
| 16MiB | C | 20 | 39.320 | 44.457 | 54.946 | 54.946 | 39.784 | 0.000 | 0.000 | 18 KiB | 22 KiB | 0.0 | - | 1 |
| 64MiB | A | 20 | 503.430 | 548.329 | 569.530 | 569.530 | 504.881 | 0.000 | 0.000 | 21 KiB | 25 KiB | 0.0 | - | 1 |
| 64MiB | B | 20 | 417.050 | 442.851 | 475.959 | 475.959 | 420.749 | 0.000 | 0.000 | 22 KiB | 26 KiB | 0.0 | 0.0% | 1 |
| 64MiB | C | 20 | 39.491 | 41.735 | 53.875 | 53.875 | 38.455 | 0.000 | 0.000 | 25 KiB | 29 KiB | 0.0 | - | 1 |
| 256MiB | A | 20 | 2183.874 | 2424.924 | 2554.858 | 2554.858 | 2247.177 | 0.000 | 0.000 | 31 KiB | 36 KiB | 0.0 | - | 1 |
| 256MiB | B | 20 | 1876.831 | 2005.121 | 2054.746 | 2054.746 | 1867.310 | 0.000 | 0.000 | 30 KiB | 35 KiB | 0.0 | 0.0% | 1 |
| 256MiB | C | 20 | 72.424 | 107.021 | 179.494 | 179.494 | 80.059 | 0.000 | 0.000 | 34 KiB | 38 KiB | 0.0 | - | 1 |

### reanchor_insert

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 20 | 20.818 | 23.087 | 28.281 | 28.281 | 20.536 | 0.000 | 0.000 | 362 KiB | 366 KiB | 0.0 | - | 2734 |
| 1MiB | B | 20 | 16.104 | 23.932 | 38.571 | 38.571 | 17.820 | 0.000 | 0.000 | 231 KiB | 235 KiB | 0.0 | 0.0% | 1604 |
| 1MiB | C | 20 | 21.367 | 25.634 | 34.439 | 34.439 | 21.311 | 0.000 | 0.000 | 375 KiB | 379 KiB | 0.0 | - | 2734 |
| 16MiB | A | 20 | 236.972 | 321.446 | 330.909 | 330.909 | 234.487 | 0.000 | 0.000 | 7.0 MiB | 7.0 MiB | 0.0 | - | 37146 |
| 16MiB | B | 20 | 188.877 | 259.940 | 374.455 | 374.455 | 198.077 | 0.000 | 0.000 | 5.5 MiB | 5.5 MiB | 0.0 | 0.0% | 30573 |
| 16MiB | C | 20 | 70.582 | 91.282 | 91.495 | 91.495 | 64.709 | 0.000 | 0.000 | 1.7 MiB | 1.8 MiB | 0.0 | - | 10368 |
| 64MiB | A | 20 | 1045.688 | 1175.810 | 1239.683 | 1239.683 | 974.390 | 0.000 | 0.000 | 39.0 MiB | 39.0 MiB | 0.0 | - | 188849 |
| 64MiB | B | 20 | 810.057 | 936.554 | 946.937 | 946.937 | 776.115 | 0.000 | 0.000 | 29.5 MiB | 29.5 MiB | 0.0 | 0.0% | 141332 |
| 64MiB | C | 20 | 73.049 | 94.237 | 94.531 | 94.531 | 68.412 | 0.000 | 0.000 | 2.0 MiB | 2.0 MiB | 0.0 | - | 9530 |
| 256MiB | A | 20 | 4343.148 | 4797.697 | 5096.886 | 5096.886 | 3981.056 | 0.000 | 0.000 | 140.3 MiB | 140.4 MiB | 0.0 | - | 681449 |
| 256MiB | B | 20 | 2715.908 | 3845.957 | 3965.054 | 3965.054 | 2718.720 | 0.000 | 0.000 | 68.2 MiB | 68.2 MiB | 0.0 | 0.0% | 327738 |
| 256MiB | C | 20 | 101.764 | 278.482 | 449.018 | 449.018 | 123.747 | 0.000 | 0.000 | 1.9 MiB | 1.9 MiB | 0.0 | - | 8983 |

### reanchor_trim

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 1 | 5.698 | 5.698 | 5.698 | 5.698 | 5.697 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 1MiB | B | 1 | 19.761 | 19.761 | 19.761 | 19.761 | 19.760 | 0.000 | 0.000 | 431 KiB | 432 KiB | 0.0 | 0.0% | 3698 |
| 1MiB | C | 1 | 5.718 | 5.718 | 5.718 | 5.718 | 5.718 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 16MiB | A | 1 | 94.376 | 94.376 | 94.376 | 94.376 | 94.376 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 16MiB | B | 1 | 240.493 | 240.493 | 240.493 | 240.493 | 240.492 | 0.000 | 0.000 | 11.3 MiB | 11.4 MiB | 0.0 | 0.0% | 54830 |
| 16MiB | C | 1 | 16.545 | 16.545 | 16.545 | 16.545 | 16.545 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 64MiB | A | 1 | 417.851 | 417.851 | 417.851 | 417.851 | 417.850 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 64MiB | B | 1 | 940.145 | 940.145 | 940.145 | 940.145 | 940.144 | 0.000 | 0.000 | 46.0 MiB | 46.0 MiB | 0.0 | 0.0% | 220858 |
| 64MiB | C | 1 | 24.951 | 24.951 | 24.951 | 24.951 | 24.950 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 256MiB | A | 1 | 2264.588 | 2264.588 | 2264.588 | 2264.588 | 2264.588 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |
| 256MiB | B | 1 | 4056.133 | 4056.133 | 4056.133 | 4056.133 | 4056.132 | 0.000 | 0.000 | 183.5 MiB | 183.6 MiB | 0.0 | 0.0% | 883108 |
| 256MiB | C | 1 | 25.267 | 25.267 | 25.267 | 25.267 | 25.266 | 0.000 | 0.000 | 0 KiB | 0 KiB | 0.0 | - | 0 |

### append_after_foreign_edit

| Size | Layout | n | p50 ms | p95 ms | p99 ms | max ms | ledger ms | file ms | scrub ms | bytes written (wchar) | to storage (write_bytes) | SQL reads | B fallback | rows changed |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1MiB | A | 5 | 13.399 | 18.553 | 18.553 | 18.553 | 7.064 | 7.279 | 0.000 | 46 KiB | 56 KiB | 2.0 | - | 0 |
| 1MiB | B | 5 | 13.778 | 15.396 | 15.396 | 15.396 | 7.073 | 7.213 | 0.000 | 35 KiB | 44 KiB | 2.0 | 0.0% | 0 |
| 1MiB | C | 5 | 13.356 | 19.327 | 19.327 | 19.327 | 7.135 | 7.257 | 0.000 | 45 KiB | 55 KiB | 2.0 | - | 0 |
| 16MiB | A | 5 | 13.184 | 55.622 | 55.622 | 55.622 | 11.775 | 10.030 | 0.000 | 48 KiB | 55 KiB | 2.0 | - | 0 |
| 16MiB | B | 5 | 13.898 | 24.236 | 24.236 | 24.236 | 8.747 | 7.009 | 0.000 | 45 KiB | 57 KiB | 2.0 | 0.0% | 0 |
| 16MiB | C | 5 | 12.975 | 26.722 | 26.722 | 26.722 | 8.037 | 7.666 | 0.000 | 41 KiB | 49 KiB | 2.0 | - | 0 |
| 64MiB | A | 5 | 12.969 | 159.765 | 159.765 | 159.765 | 29.054 | 13.654 | 0.000 | 60 KiB | 70 KiB | 2.0 | - | 0 |
| 64MiB | B | 5 | 13.175 | 55.145 | 55.145 | 55.145 | 15.021 | 6.511 | 0.000 | 50 KiB | 60 KiB | 2.0 | 0.0% | 0 |
| 64MiB | C | 5 | 14.681 | 34.396 | 34.396 | 34.396 | 11.419 | 6.930 | 0.000 | 55 KiB | 64 KiB | 2.0 | - | 0 |
| 256MiB | A | 5 | 14.864 | 23.737 | 23.737 | 23.737 | 9.189 | 6.989 | 0.000 | 56 KiB | 67 KiB | 2.0 | - | 0 |
| 256MiB | B | 5 | 13.538 | 23.989 | 23.989 | 23.989 | 8.860 | 6.657 | 0.000 | 50 KiB | 61 KiB | 2.0 | 0.0% | 0 |
| 256MiB | C | 5 | 13.680 | 25.659 | 25.659 | 25.659 | 9.062 | 6.831 | 0.000 | 48 KiB | 56 KiB | 2.0 | - | 0 |

### History after 1,000 edits

- 1MiB A: edits=1000; secs=14.4; ledger_bytes 1957888 -> 2207744 (+249856); versions 4919 -> 5919 (+1000); version_body_bytes +171523; file_bytes 1044532 -> 1063137 (+18605)
- 1MiB B: edits=1000; secs=10.8; ledger_bytes 1589248 -> 1835008 (+245760); versions 4009 -> 5009 (+1000); version_body_bytes +160102; file_bytes 1118671 -> 1144037 (+25366)
- 1MiB C: edits=1000; secs=14.3; ledger_bytes 1978368 -> 2228224 (+249856); versions 4919 -> 5919 (+1000); version_body_bytes +171523; file_bytes 1044549 -> 1063154 (+18605)
- 16MiB A: edits=1000; secs=14.9; ledger_bytes 26947584 -> 27226112 (+278528); versions 68960 -> 69960 (+1000); version_body_bytes +182900; file_bytes 16759869 -> 16786128 (+26259)
- 16MiB B: edits=1000; secs=11.4; ledger_bytes 21651456 -> 21893120 (+241664); versions 55168 -> 56168 (+1000); version_body_bytes +156981; file_bytes 16845234 -> 16867964 (+22730)
- 16MiB C: edits=1000; secs=15.4; ledger_bytes 27189248 -> 27443200 (+253952); versions 68965 -> 69965 (+1000); version_body_bytes +166792; file_bytes 16758507 -> 16778213 (+19706)
- 64MiB A: edits=1000; secs=15.0; ledger_bytes 107249664 -> 107520000 (+270336); versions 276050 -> 277050 (+1000); version_body_bytes +183701; file_bytes 67098073 -> 67119416 (+21343)
- 64MiB B: edits=1000; secs=11.4; ledger_bytes 86335488 -> 86605824 (+270336); versions 221199 -> 222199 (+1000); version_body_bytes +174336; file_bytes 67192100 -> 67222159 (+30059)
- 64MiB C: edits=1000; secs=14.9; ledger_bytes 107962368 -> 108224512 (+262144); versions 276044 -> 277044 (+1000); version_body_bytes +170197; file_bytes 67089225 -> 67104147 (+14922)
- 256MiB A: edits=1000; secs=14.6; ledger_bytes 425840640 -> 426119168 (+278528); versions 1103598 -> 1104598 (+1000); version_body_bytes +185550; file_bytes 268422281 -> 268445912 (+23631)
- 256MiB B: edits=1000; secs=11.3; ledger_bytes 342138880 -> 342413312 (+274432); versions 883446 -> 884446 (+1000); version_body_bytes +179123; file_bytes 268509502 -> 268533545 (+24043)
- 256MiB C: edits=1000; secs=17.7; ledger_bytes 428789760 -> 429027328 (+237568); versions 1103599 -> 1104599 (+1000); version_body_bytes +156718; file_bytes 268408694 -> 268427903 (+19209)

### Final check (file equals ledger)

- 1MiB A: ok 0.0s
- 1MiB A2: ok 0.0s
- 1MiB B: ok 0.0s
- 1MiB C: ok 0.0s
- 16MiB A: ok 0.6s
- 16MiB A2: ok 0.6s
- 16MiB B: ok 0.5s
- 16MiB C: ok 0.6s
- 64MiB A: ok 2.3s
- 64MiB A2: ok 2.3s
- 64MiB B: ok 1.9s
- 64MiB C: ok 2.3s
- 256MiB A: ok 9.7s
- 256MiB A2: ok 9.5s
- 256MiB B: ok 7.6s
- 256MiB C: ok 9.9s

### Syscalls per operation (strace, 20 samples each)

| Case | Operation | flushes (fsync+fdatasync) | reads | writes | detail |
| --- | --- | ---: | ---: | ---: | --- |
| 1-A | append | 2.08 | 0.00 | 20.92 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=17.88, statx[-]=1.00, statx[chat]=1.00 |
| 1-A | edit_random | 2.15 | 9.10 | 266.45 | fdatasync[chat]=1.00, fstat[temp]=0.95, fstat[ledger.db]=0.05, fsync[ledger.db]=0.05, fsync[ledger.db-wal]=1.10, ftruncate[chat]=0.35, ftruncate[ledger.db]=0.05, newfstatat[-]=0.95, openat[-]=0.95, pread64[chat]=1.00, pread64[ledger.db-wal]=8.10, pwrite64[temp]=108.60, pwrite64[ledger.db]=8.10, pwrite64[ledger.db-wal]=148.75, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-A | edit_recent | 2.00 | 1.00 | 26.50 | fdatasync[chat]=1.00, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.50, pread64[chat]=1.00, pwrite64[ledger.db-wal]=25.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-A | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 1-A | reanchor_insert | 1.00 | 2.00 | 125.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pwrite64[ledger.db-wal]=125.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-A | reanchor_trim | 0.00 | 2.00 | 0.00 | openat[-]=2.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-A | reanchor_typo | 1.50 | 2.00 | 9.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pwrite64[ledger.db-wal]=9.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-A | redact_random | 5.00 | 79.60 | 358.80 | fdatasync[chat]=1.00, fstat[temp]=0.90, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.90, openat[-]=0.90, pread64[chat]=1.00, pread64[ledger.db-wal]=78.60, pwrite64[temp]=119.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=78.60, pwrite64[ledger.db-wal]=158.20, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-A | redact_recent | 4.95 | 20.75 | 52.50 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[chat]=1.00, pread64[ledger.db-wal]=19.75, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=19.75, pwrite64[ledger.db-wal]=29.75, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-A | unread_after_p | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 1-A2 | edit_random | 4.85 | 8.70 | 198.50 | fadvise64[chat]=1.70, fdatasync[chat]=1.70, fstat[temp]=0.85, fstat[ledger.db]=0.05, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db]=0.05, fsync[ledger.db-wal]=1.10, ftruncate[ledger.db]=0.05, newfstatat[-]=0.85, openat[-]=4.85, pread64[ledger.db-wal]=6.70, pwrite64[temp]=75.30, pwrite64[ledger.db]=6.70, pwrite64[ledger.db-wal]=114.65, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=1.85 |
| 1-A2 | edit_recent | 5.05 | 2.00 | 25.95 | fadvise64[chat]=2.00, fdatasync[chat]=2.00, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db-wal]=1.05, openat[-]=4.00, pwrite64[ledger.db-wal]=23.95, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=2.00 |
| 1-B | append | 2.08 | 0.00 | 19.96 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=16.92, statx[-]=1.00, statx[chat]=1.00 |
| 1-B | edit_random | 2.00 | 0.05 | 23.90 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=0.05, pwrite64[chat]=0.95, pwrite64[temp]=4.70, pwrite64[ledger.db-wal]=18.20, statx[-]=1.00, statx[chat]=1.00, write[chat]=0.05 |
| 1-B | edit_recent | 2.00 | 0.10 | 14.20 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=0.10, pwrite64[chat]=0.90, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=13.10, statx[-]=1.00, statx[chat]=1.00, write[chat]=0.10 |
| 1-B | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 1-B | reanchor_insert | 1.00 | 2.00 | 88.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pwrite64[ledger.db-wal]=88.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-B | reanchor_trim | 1.00 | 2.00 | 196.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pwrite64[ledger.db-wal]=196.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-B | reanchor_typo | 1.50 | 2.00 | 9.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pwrite64[ledger.db-wal]=9.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-B | redact_random | 5.00 | 6.25 | 22.75 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db-wal]=6.25, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=6.25, pwrite64[ledger.db-wal]=13.50, statx[-]=1.00, statx[chat]=1.00 |
| 1-B | redact_recent | 4.95 | 11.60 | 28.25 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db-wal]=11.60, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=11.60, pwrite64[ledger.db-wal]=13.65, statx[-]=1.00, statx[chat]=1.00 |
| 1-B | unread_after_p | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 1-C | append | 2.08 | 0.00 | 21.00 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=17.96, statx[-]=1.00, statx[chat]=1.00 |
| 1-C | edit_random | 2.25 | 15.95 | 293.30 | fdatasync[chat]=1.00, fstat[temp]=1.00, fstat[ledger.db]=0.10, fsync[ledger.db]=0.10, fsync[ledger.db-wal]=1.15, ftruncate[chat]=0.35, ftruncate[ledger.db]=0.10, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[ledger.db-wal]=14.95, pwrite64[temp]=118.70, pwrite64[ledger.db]=14.95, pwrite64[ledger.db-wal]=158.65, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-C | edit_recent | 2.00 | 1.00 | 29.50 | fdatasync[chat]=1.00, fstat[temp]=0.15, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.50, newfstatat[-]=0.15, openat[-]=0.15, pread64[chat]=1.00, pwrite64[temp]=0.40, pwrite64[ledger.db-wal]=28.10, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-C | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 1-C | reanchor_insert | 1.00 | 2.00 | 132.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pwrite64[ledger.db-wal]=132.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-C | reanchor_trim | 0.00 | 2.00 | 0.00 | openat[-]=2.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-C | reanchor_typo | 1.50 | 2.00 | 9.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pwrite64[ledger.db-wal]=9.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 1-C | redact_random | 5.00 | 83.10 | 376.50 | fdatasync[chat]=1.00, fstat[temp]=0.90, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.90, openat[-]=0.90, pread64[chat]=1.00, pread64[ledger.db-wal]=82.10, pwrite64[temp]=126.20, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=82.10, pwrite64[ledger.db-wal]=165.20, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-C | redact_recent | 5.00 | 16.85 | 52.45 | fdatasync[chat]=1.00, fstat[temp]=0.25, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.25, openat[-]=0.25, pread64[chat]=1.00, pread64[ledger.db-wal]=15.85, pwrite64[temp]=0.90, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=15.85, pwrite64[ledger.db-wal]=32.70, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 1-C | unread_after_p | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 16-A | append | 2.08 | 3.24 | 26.76 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pread64[ledger.db]=3.16, pread64[ledger.db-wal]=0.08, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=23.72, statx[-]=1.00, statx[chat]=1.00 |
| 16-A | edit_random | 10.60 | 1222.85 | 3665.05 | fadvise64[chat]=7.25, fdatasync[chat]=8.25, fstat[temp]=0.95, fstat[ledger.db]=0.45, fsync[ledger.db]=0.45, fsync[ledger.db-wal]=1.90, ftruncate[chat]=0.45, ftruncate[ledger.db]=0.45, newfstatat[-]=0.95, openat[-]=0.95, pread64[chat]=1.00, pread64[ledger.db]=527.25, pread64[ledger.db-wal]=694.60, pwrite64[temp]=1514.70, pwrite64[ledger.db]=589.85, pwrite64[ledger.db-wal]=1553.15, statx[-]=1.00, statx[chat]=1.00, write[chat]=7.35 |
| 16-A | edit_recent | 2.00 | 1.95 | 28.50 | fdatasync[chat]=1.00, fstat[temp]=0.10, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.40, newfstatat[-]=0.10, openat[-]=0.10, pread64[chat]=1.00, pread64[ledger.db]=0.95, pwrite64[temp]=0.20, pwrite64[ledger.db-wal]=27.30, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 16-A | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 16-A | reanchor_insert | 1.00 | 2098.00 | 825.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pread64[ledger.db]=1778.00, pread64[ledger.db-wal]=318.00, pwrite64[ledger.db-wal]=825.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-A | reanchor_trim | 0.00 | 1782.00 | 0.00 | openat[-]=2.00, pread64[ledger.db]=1153.00, pread64[ledger.db-wal]=627.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-A | reanchor_typo | 1.50 | 1791.50 | 15.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=1787.00, pread64[ledger.db-wal]=2.50, pwrite64[ledger.db-wal]=15.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-A | redact_random | 12.75 | 2358.10 | 4926.85 | fadvise64[chat]=7.75, fdatasync[chat]=8.75, fstat[temp]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[index.db]=0.10, pread64[ledger.db]=736.55, pread64[ledger.db-wal]=1620.45, pwrite64[temp]=1629.50, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=833.80, pwrite64[ledger.db-wal]=2453.80, statx[-]=1.00, statx[chat]=1.00, write[chat]=7.75 |
| 16-A | redact_recent | 4.95 | 45.90 | 75.75 | fdatasync[chat]=1.00, fstat[temp]=0.05, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[chat]=0.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=1.00, pread64[ledger.db]=1.25, pread64[ledger.db-wal]=43.65, pwrite64[temp]=0.20, pwrite64[index.db-wal]=1.90, pwrite64[ledger.db]=43.60, pwrite64[ledger.db-wal]=29.05, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 16-A | unread_after_p | 0.00 | 2.85 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=1.85, statx[-]=1.00 |
| 16-A2 | edit_random | 21.60 | 1295.75 | 3920.05 | fadvise64[chat]=17.00, fdatasync[chat]=17.00, fstat[temp]=1.00, fstat[ledger.db]=0.55, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db]=0.55, fsync[ledger.db-wal]=2.05, ftruncate[ledger.db]=0.55, newfstatat[-]=1.00, openat[-]=5.00, pread64[ledger.db]=511.80, pread64[ledger.db-wal]=781.95, pwrite64[temp]=1636.70, pwrite64[ledger.db]=591.55, pwrite64[ledger.db-wal]=1674.80, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=17.00 |
| 16-A2 | edit_recent | 20.00 | 3.20 | 45.70 | fadvise64[chat]=16.95, fdatasync[chat]=16.95, fstat[temp]=0.10, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db-wal]=1.05, newfstatat[-]=0.10, openat[-]=4.10, pread64[ledger.db]=1.20, pwrite64[temp]=0.20, pwrite64[ledger.db-wal]=28.55, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=16.95 |
| 16-B | append | 2.12 | 3.12 | 26.56 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.08, pread64[ledger.db]=3.12, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=23.52, statx[-]=1.00, statx[chat]=1.00 |
| 16-B | edit_random | 3.50 | 192.50 | 533.55 | fadvise64[chat]=1.20, fdatasync[chat]=2.20, fstat[temp]=0.15, fstat[ledger.db]=0.10, fsync[ledger.db]=0.10, fsync[ledger.db-wal]=1.20, ftruncate[ledger.db]=0.10, newfstatat[-]=0.15, openat[-]=0.15, pread64[chat]=0.15, pread64[ledger.db]=87.95, pread64[ledger.db-wal]=104.40, pwrite64[chat]=0.85, pwrite64[temp]=203.60, pwrite64[ledger.db]=103.20, pwrite64[ledger.db-wal]=224.70, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.20 |
| 16-B | edit_recent | 2.00 | 1.05 | 15.80 | fdatasync[chat]=1.00, fsync[ledger.db-wal]=1.00, pread64[chat]=0.25, pread64[ledger.db]=0.80, pwrite64[chat]=0.75, pwrite64[ledger.db-wal]=14.80, statx[-]=1.00, statx[chat]=1.00, write[chat]=0.25 |
| 16-B | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 16-B | reanchor_insert | 2.50 | 2671.50 | 2261.00 | fstat[ledger.db]=0.50, fsync[ledger.db]=0.50, fsync[ledger.db-wal]=2.00, ftruncate[ledger.db]=0.50, openat[-]=2.00, pread64[ledger.db]=2043.00, pread64[ledger.db-wal]=626.50, pwrite64[ledger.db]=620.50, pwrite64[ledger.db-wal]=1640.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-B | reanchor_trim | 3.00 | 4291.00 | 4286.00 | fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.00, ftruncate[ledger.db]=1.00, openat[-]=2.00, pread64[ledger.db]=2048.00, pread64[ledger.db-wal]=2241.00, pwrite64[ledger.db]=1430.00, pwrite64[ledger.db-wal]=2856.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-B | reanchor_typo | 1.50 | 1433.00 | 18.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=1428.00, pread64[ledger.db-wal]=3.00, pwrite64[ledger.db-wal]=18.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-B | redact_random | 5.00 | 9.85 | 28.60 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db]=1.65, pread64[ledger.db-wal]=8.20, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=8.20, pwrite64[ledger.db-wal]=17.40, statx[-]=1.00, statx[chat]=1.00 |
| 16-B | redact_recent | 4.95 | 7.65 | 22.75 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db]=0.95, pread64[ledger.db-wal]=6.70, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=6.70, pwrite64[ledger.db-wal]=13.05, statx[-]=1.00, statx[chat]=1.00 |
| 16-B | unread_after_p | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 16-C | append | 2.08 | 3.28 | 26.68 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pread64[ledger.db]=3.12, pread64[ledger.db-wal]=0.16, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=23.64, statx[-]=1.00, statx[chat]=1.00 |
| 16-C | edit_random | 5.75 | 414.35 | 1331.65 | fadvise64[chat]=3.00, fdatasync[chat]=4.00, fstat[temp]=1.00, fstat[ledger.db]=0.25, fsync[ledger.db]=0.25, fsync[ledger.db-wal]=1.50, ftruncate[chat]=0.30, ftruncate[ledger.db]=0.25, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[ledger.db]=195.40, pread64[ledger.db-wal]=217.95, pwrite64[temp]=542.60, pwrite64[ledger.db]=196.70, pwrite64[ledger.db-wal]=589.25, statx[-]=1.00, statx[chat]=1.00, write[chat]=3.10 |
| 16-C | edit_recent | 2.00 | 1.95 | 31.00 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.35, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=1.00, pread64[ledger.db]=0.95, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=29.90, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 16-C | read_last_50 | 0.00 | 1.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=0.05, statx[-]=1.00 |
| 16-C | reanchor_insert | 1.00 | 495.00 | 387.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pread64[ledger.db]=490.50, pread64[ledger.db-wal]=2.50, pwrite64[ledger.db-wal]=387.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-C | reanchor_trim | 0.00 | 292.00 | 0.00 | openat[-]=2.00, pread64[ledger.db]=283.00, pread64[ledger.db-wal]=7.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-C | reanchor_typo | 1.50 | 501.00 | 9.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=498.00, pread64[ledger.db-wal]=1.00, pwrite64[ledger.db-wal]=9.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 16-C | redact_random | 7.45 | 473.05 | 1244.65 | fadvise64[chat]=2.45, fdatasync[chat]=3.45, fstat[temp]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[index.db]=0.25, pread64[ledger.db]=197.60, pread64[ledger.db-wal]=274.20, pwrite64[temp]=460.30, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=252.20, pwrite64[ledger.db-wal]=527.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=2.65 |
| 16-C | redact_recent | 4.95 | 30.75 | 62.25 | fdatasync[chat]=1.00, fstat[temp]=0.10, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[chat]=0.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.10, openat[-]=0.10, pread64[chat]=1.00, pread64[ledger.db]=1.55, pread64[ledger.db-wal]=28.20, pwrite64[temp]=0.20, pwrite64[index.db-wal]=1.90, pwrite64[ledger.db]=28.20, pwrite64[ledger.db-wal]=30.95, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 16-C | unread_after_p | 0.00 | 3.20 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.20, statx[-]=1.00 |
| 64-A | append | 2.08 | 4.24 | 28.12 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pread64[ledger.db]=4.16, pread64[ledger.db-wal]=0.08, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=25.08, statx[-]=1.00, statx[chat]=1.00 |
| 64-A | edit_random | 35.10 | 7454.85 | 17277.05 | fadvise64[chat]=30.45, fdatasync[chat]=31.45, fstat[temp]=1.05, fstat[ledger.db]=0.90, fsync[ledger.db]=0.90, fsync[ledger.db-wal]=2.75, ftruncate[chat]=0.55, ftruncate[ledger.db]=0.90, newfstatat[-]=1.05, openat[-]=1.05, pread64[chat]=1.00, pread64[ledger.db]=3766.75, pread64[ledger.db-wal]=3687.10, pwrite64[temp]=6786.65, pwrite64[ledger.db]=3372.65, pwrite64[ledger.db-wal]=7087.30, statx[-]=1.00, statx[chat]=1.00, write[chat]=30.45 |
| 64-A | edit_recent | 2.00 | 2.30 | 29.90 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.55, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=1.00, pread64[ledger.db]=1.30, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=28.80, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 64-A | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 64-A | reanchor_insert | 2.50 | 10287.00 | 4608.50 | fstat[ledger.db]=0.50, fsync[ledger.db]=0.50, fsync[ledger.db-wal]=2.00, ftruncate[ledger.db]=0.50, openat[-]=2.00, pread64[ledger.db]=8758.00, pread64[ledger.db-wal]=1527.00, pwrite64[ledger.db]=1523.00, pwrite64[ledger.db-wal]=3085.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-A | reanchor_trim | 0.00 | 7235.00 | 0.00 | openat[-]=2.00, pread64[ledger.db]=7192.00, pread64[ledger.db-wal]=41.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-A | reanchor_typo | 1.50 | 7246.00 | 14.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=7242.00, pread64[ledger.db-wal]=2.00, pwrite64[ledger.db-wal]=14.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-A | redact_random | 39.15 | 11788.75 | 22805.15 | fadvise64[chat]=34.15, fdatasync[chat]=35.15, fstat[temp]=1.10, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.10, openat[-]=1.10, pread64[chat]=1.00, pread64[index.db]=0.90, pread64[ledger.db]=4281.30, pread64[ledger.db-wal]=7505.55, pwrite64[temp]=7612.80, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=3825.60, pwrite64[ledger.db-wal]=11330.60, statx[-]=1.00, statx[chat]=1.00, write[chat]=34.15 |
| 64-A | redact_recent | 5.00 | 17.25 | 48.00 | fdatasync[chat]=1.00, fstat[temp]=0.10, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.10, openat[-]=0.10, pread64[chat]=1.00, pread64[ledger.db]=1.65, pread64[ledger.db-wal]=14.60, pwrite64[temp]=0.20, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=14.60, pwrite64[ledger.db-wal]=30.20, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 64-A | unread_after_p | 0.00 | 3.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.05, statx[-]=1.00 |
| 64-A2 | edit_random | 70.60 | 9568.80 | 22191.55 | fadvise64[chat]=64.80, fdatasync[chat]=64.80, fstat[temp]=1.20, fstat[ledger.db]=0.95, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db]=0.95, fsync[ledger.db-wal]=2.85, ftruncate[ledger.db]=0.95, newfstatat[-]=1.20, openat[-]=5.20, pread64[ledger.db]=4919.20, pread64[ledger.db-wal]=4647.60, pwrite64[temp]=8729.00, pwrite64[ledger.db]=4369.90, pwrite64[ledger.db-wal]=9027.85, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=64.80 |
| 64-A2 | edit_recent | 68.05 | 4.35 | 93.35 | fadvise64[chat]=65.00, fdatasync[chat]=65.00, fstat[temp]=0.05, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db-wal]=1.05, newfstatat[-]=0.05, openat[-]=4.05, pread64[ledger.db]=2.35, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=28.25, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=65.00 |
| 64-B | append | 2.12 | 4.12 | 27.84 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.08, pread64[ledger.db]=4.12, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=24.80, statx[-]=1.00, statx[chat]=1.00 |
| 64-B | edit_random | 10.15 | 1430.90 | 3467.70 | fadvise64[chat]=7.70, fdatasync[chat]=8.70, fstat[temp]=0.25, fstat[ledger.db]=0.15, fsync[ledger.db]=0.15, fsync[ledger.db-wal]=1.30, ftruncate[ledger.db]=0.15, newfstatat[-]=0.25, openat[-]=0.25, pread64[chat]=0.25, pread64[ledger.db]=757.85, pread64[ledger.db-wal]=672.80, pwrite64[chat]=0.75, pwrite64[temp]=1388.20, pwrite64[ledger.db]=657.30, pwrite64[ledger.db-wal]=1413.75, statx[-]=1.00, statx[chat]=1.00, write[chat]=7.70 |
| 64-B | edit_recent | 2.00 | 1.00 | 14.30 | fdatasync[chat]=1.00, fsync[ledger.db-wal]=1.00, pread64[chat]=0.20, pread64[ledger.db]=0.80, pwrite64[chat]=0.80, pwrite64[ledger.db-wal]=13.30, statx[-]=1.00, statx[chat]=1.00, write[chat]=0.20 |
| 64-B | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 64-B | reanchor_insert | 2.00 | 11653.50 | 8849.00 | fstat[ledger.db]=0.50, fsync[ledger.db]=0.50, fsync[ledger.db-wal]=1.50, ftruncate[ledger.db]=0.50, openat[-]=2.00, pread64[ledger.db]=8590.50, pread64[ledger.db-wal]=3061.00, pwrite64[ledger.db]=2902.00, pwrite64[ledger.db-wal]=5947.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-B | reanchor_trim | 4.00 | 17527.00 | 17503.00 | fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, openat[-]=2.00, pread64[ledger.db]=11691.00, pread64[ledger.db-wal]=5834.00, pwrite64[ledger.db]=5834.00, pwrite64[ledger.db-wal]=11669.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-B | reanchor_typo | 1.50 | 5859.50 | 20.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=5855.00, pread64[ledger.db-wal]=2.50, pwrite64[ledger.db-wal]=20.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-B | redact_random | 5.00 | 14.75 | 35.95 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[index.db]=0.90, pread64[ledger.db]=3.20, pread64[ledger.db-wal]=10.65, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=10.65, pwrite64[ledger.db-wal]=22.30, statx[-]=1.00, statx[chat]=1.00 |
| 64-B | redact_recent | 4.95 | 44.80 | 59.80 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db]=1.35, pread64[ledger.db-wal]=43.45, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=43.45, pwrite64[ledger.db-wal]=13.35, statx[-]=1.00, statx[chat]=1.00 |
| 64-B | unread_after_p | 0.00 | 3.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.05, statx[-]=1.00 |
| 64-C | append | 2.08 | 4.32 | 27.88 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pread64[ledger.db]=4.28, pread64[ledger.db-wal]=0.04, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=24.84, statx[-]=1.00, statx[chat]=1.00 |
| 64-C | edit_random | 5.25 | 461.95 | 1260.70 | fadvise64[chat]=2.65, fdatasync[chat]=3.65, fstat[temp]=0.90, fstat[ledger.db]=0.20, fsync[ledger.db]=0.20, fsync[ledger.db-wal]=1.40, ftruncate[chat]=0.15, ftruncate[ledger.db]=0.20, newfstatat[-]=0.90, openat[-]=0.90, pread64[chat]=1.00, pread64[ledger.db]=230.00, pread64[ledger.db-wal]=230.95, pwrite64[temp]=501.40, pwrite64[ledger.db]=210.70, pwrite64[ledger.db-wal]=545.70, statx[-]=1.00, statx[chat]=1.00, write[chat]=2.90 |
| 64-C | edit_recent | 2.00 | 2.05 | 27.80 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.40, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=1.00, pread64[ledger.db]=1.05, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=26.70, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 64-C | read_last_50 | 0.00 | 1.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=0.05, statx[-]=1.00 |
| 64-C | reanchor_insert | 1.00 | 517.50 | 238.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pread64[ledger.db]=513.00, pread64[ledger.db-wal]=2.50, pwrite64[ledger.db-wal]=238.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-C | reanchor_trim | 1.00 | 493.00 | 2.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pread64[ledger.db]=490.00, pread64[ledger.db-wal]=1.00, pwrite64[ledger.db-wal]=2.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-C | reanchor_typo | 1.50 | 508.00 | 20.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=505.00, pread64[ledger.db-wal]=1.00, pwrite64[ledger.db-wal]=20.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 64-C | redact_random | 7.05 | 542.15 | 1219.75 | fadvise64[chat]=2.05, fdatasync[chat]=3.05, fstat[temp]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[index.db]=0.95, pread64[ledger.db]=231.50, pread64[ledger.db-wal]=308.70, pwrite64[temp]=430.60, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=237.50, pwrite64[ledger.db-wal]=547.20, statx[-]=1.00, statx[chat]=1.00, write[chat]=2.45 |
| 64-C | redact_recent | 4.95 | 59.70 | 87.30 | fdatasync[chat]=1.00, fstat[temp]=0.05, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[chat]=0.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=1.00, pread64[ledger.db]=1.95, pread64[ledger.db-wal]=56.75, pwrite64[temp]=0.10, pwrite64[index.db-wal]=1.90, pwrite64[ledger.db]=56.75, pwrite64[ledger.db-wal]=27.55, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 64-C | unread_after_p | 0.00 | 3.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.05, statx[-]=1.00 |
| 256-A | append | 2.12 | 5.60 | 32.24 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.08, pread64[ledger.db]=5.60, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.04, pwrite64[ledger.db-wal]=29.20, statx[-]=1.00, statx[chat]=1.00 |
| 256-A | edit_random | 151.35 | 36695.20 | 81189.40 | fadvise64[chat]=146.55, fdatasync[chat]=147.55, fstat[temp]=1.70, fstat[ledger.db]=0.95, fsync[ledger.db]=0.95, fsync[ledger.db-wal]=2.85, ftruncate[chat]=0.50, ftruncate[ledger.db]=0.95, newfstatat[-]=1.70, openat[-]=1.70, pread64[chat]=1.00, pread64[temp]=525.20, pread64[ledger.db]=17542.25, pread64[ledger.db-wal]=18626.75, pwrite64[temp]=31452.10, pwrite64[ledger.db]=15488.70, pwrite64[ledger.db-wal]=34102.05, statx[-]=1.00, statx[chat]=1.00, write[chat]=146.55 |
| 256-A | edit_recent | 2.00 | 2.25 | 26.10 | fdatasync[chat]=1.00, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.20, pread64[chat]=1.00, pread64[ledger.db]=1.25, pwrite64[ledger.db-wal]=25.10, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 256-A | read_last_50 | 0.00 | 1.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=0.05, statx[-]=1.00 |
| 256-A | reanchor_insert | 3.50 | 44346.00 | 22938.00 | fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.50, ftruncate[ledger.db]=1.00, openat[-]=2.00, pread64[ledger.db]=36690.50, pread64[ledger.db-wal]=7653.50, pwrite64[ledger.db]=7650.50, pwrite64[ledger.db-wal]=15287.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-A | reanchor_trim | 0.00 | 29037.00 | 0.00 | openat[-]=2.00, pread64[ledger.db]=29035.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-A | reanchor_typo | 1.50 | 29049.00 | 19.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=29045.00, pread64[ledger.db-wal]=2.00, pwrite64[ledger.db-wal]=19.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-A | redact_random | 125.35 | 43036.85 | 82152.35 | fadvise64[chat]=120.35, fdatasync[chat]=121.35, fstat[temp]=1.75, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.75, openat[-]=1.75, pread64[chat]=1.00, pread64[temp]=444.55, pread64[index.db]=1.50, pread64[ledger.db]=15395.55, pread64[ledger.db-wal]=27194.25, pwrite64[temp]=27621.35, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=13607.65, pwrite64[ledger.db-wal]=40801.00, statx[-]=1.00, statx[chat]=1.00, write[chat]=120.35 |
| 256-A | redact_recent | 5.00 | 17.30 | 47.50 | fdatasync[chat]=1.00, fstat[temp]=0.15, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.15, openat[-]=0.15, pread64[chat]=0.95, pread64[ledger.db]=1.95, pread64[ledger.db-wal]=14.40, pwrite64[temp]=0.30, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=14.40, pwrite64[ledger.db-wal]=29.80, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 256-A | unread_after_p | 0.00 | 3.10 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.10, statx[-]=1.00 |
| 256-A2 | edit_random | 262.95 | 31561.05 | 70240.25 | fadvise64[chat]=257.00, fdatasync[chat]=257.00, fstat[temp]=1.70, fstat[ledger.db]=1.00, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[ledger.db]=1.00, newfstatat[-]=1.70, openat[-]=5.70, pread64[temp]=434.25, pread64[ledger.db]=15182.30, pread64[ledger.db-wal]=15942.50, pwrite64[temp]=27218.45, pwrite64[ledger.db]=13411.50, pwrite64[ledger.db-wal]=29353.30, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=257.00 |
| 256-A2 | edit_recent | 260.05 | 4.40 | 282.25 | fadvise64[chat]=257.00, fdatasync[chat]=257.00, fsync[chat]=1.00, fsync[temp]=1.00, fsync[ledger.db-wal]=1.05, openat[-]=4.00, pread64[ledger.db]=2.40, pwrite64[ledger.db-wal]=25.25, read[chat]=2.00, rename[-]=1.00, statx[-]=1.00, statx[chat]=2.00, write[chat]=257.00 |
| 256-B | append | 2.12 | 5.28 | 31.68 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.08, pread64[ledger.db]=5.28, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.20, pwrite64[ledger.db-wal]=28.48, statx[-]=1.00, statx[chat]=1.00 |
| 256-B | edit_random | 30.35 | 6554.75 | 13849.25 | fadvise64[chat]=27.65, fdatasync[chat]=28.65, fstat[temp]=0.45, fstat[ledger.db]=0.25, fsync[ledger.db]=0.25, fsync[ledger.db-wal]=1.45, ftruncate[ledger.db]=0.25, newfstatat[-]=0.45, openat[-]=0.45, pread64[chat]=0.25, pread64[temp]=58.35, pread64[ledger.db]=2863.50, pread64[ledger.db-wal]=3632.65, pwrite64[chat]=0.75, pwrite64[temp]=5111.15, pwrite64[ledger.db]=2542.20, pwrite64[ledger.db-wal]=6167.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=27.65 |
| 256-B | edit_recent | 2.00 | 1.55 | 17.60 | fdatasync[chat]=1.00, fstat[temp]=0.05, fsync[ledger.db-wal]=1.00, newfstatat[-]=0.05, openat[-]=0.05, pread64[chat]=0.25, pread64[ledger.db]=1.30, pwrite64[chat]=0.75, pwrite64[temp]=0.10, pwrite64[ledger.db-wal]=16.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=0.25 |
| 256-B | read_last_50 | 0.00 | 1.00 | 0.00 | pread64[chat]=1.00, statx[-]=1.00 |
| 256-B | reanchor_insert | 3.50 | 53547.50 | 45107.50 | fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.50, ftruncate[ledger.db]=1.00, openat[-]=2.00, pread64[ledger.db]=38501.50, pread64[ledger.db-wal]=15044.00, pwrite64[ledger.db]=15039.00, pwrite64[ledger.db-wal]=30068.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-B | reanchor_trim | 4.00 | 91763.00 | 91670.00 | fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, openat[-]=2.00, pread64[ledger.db]=46887.00, pread64[ledger.db-wal]=44874.00, pwrite64[ledger.db]=23398.00, pwrite64[ledger.db-wal]=68272.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-B | reanchor_typo | 1.50 | 23455.50 | 19.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=23451.50, pread64[ledger.db-wal]=2.00, pwrite64[ledger.db-wal]=19.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-B | redact_random | 5.00 | 16.45 | 38.35 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[index.db]=1.25, pread64[ledger.db]=3.75, pread64[ledger.db-wal]=11.45, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=11.45, pwrite64[ledger.db-wal]=23.90, statx[-]=1.00, statx[chat]=1.00 |
| 256-B | redact_recent | 5.00 | 8.30 | 23.35 | fdatasync[chat]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, pread64[ledger.db]=1.85, pread64[ledger.db-wal]=6.45, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=6.45, pwrite64[ledger.db-wal]=13.90, statx[-]=1.00, statx[chat]=1.00 |
| 256-B | unread_after_p | 0.00 | 3.15 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.15, statx[-]=1.00 |
| 256-C | append | 2.08 | 5.48 | 31.64 | fdatasync[chat]=1.00, fsync[index.db-wal]=0.04, fsync[ledger.db-wal]=1.04, pread64[ledger.db]=5.40, pread64[ledger.db-wal]=0.08, pwrite64[chat]=1.00, pwrite64[index.db-wal]=2.20, pwrite64[ledger.db-wal]=28.44, statx[-]=1.00, statx[chat]=1.00 |
| 256-C | edit_random | 4.50 | 432.10 | 1081.15 | fadvise64[chat]=1.90, fdatasync[chat]=2.90, fstat[temp]=0.95, fstat[ledger.db]=0.20, fsync[ledger.db]=0.20, fsync[ledger.db-wal]=1.40, ftruncate[chat]=0.40, ftruncate[ledger.db]=0.20, newfstatat[-]=0.95, openat[-]=0.95, pread64[chat]=1.00, pread64[ledger.db]=222.90, pread64[ledger.db-wal]=208.20, pwrite64[temp]=411.20, pwrite64[ledger.db]=208.15, pwrite64[ledger.db-wal]=459.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=2.30 |
| 256-C | edit_recent | 2.00 | 2.45 | 26.50 | fdatasync[chat]=1.00, fsync[ledger.db-wal]=1.00, ftruncate[chat]=0.45, pread64[chat]=1.00, pread64[ledger.db]=1.45, pwrite64[ledger.db-wal]=25.50, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 256-C | read_last_50 | 0.00 | 1.05 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=0.05, statx[-]=1.00 |
| 256-C | reanchor_insert | 1.00 | 490.00 | 377.00 | fsync[ledger.db-wal]=1.00, openat[-]=2.00, pread64[ledger.db]=485.00, pread64[ledger.db-wal]=3.00, pwrite64[ledger.db-wal]=377.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-C | reanchor_trim | 0.00 | 491.00 | 0.00 | openat[-]=2.00, pread64[ledger.db]=485.00, pread64[ledger.db-wal]=4.00, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-C | reanchor_typo | 1.50 | 510.00 | 21.50 | fsync[ledger.db-wal]=1.50, openat[-]=2.00, pread64[ledger.db]=507.00, pread64[ledger.db-wal]=1.00, pwrite64[ledger.db-wal]=21.50, read[chat]=2.00, statx[-]=1.00, statx[chat]=2.00 |
| 256-C | redact_random | 7.75 | 633.10 | 1487.95 | fadvise64[chat]=2.75, fdatasync[chat]=3.75, fstat[temp]=1.00, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=3.00, ftruncate[chat]=1.00, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=1.00, openat[-]=1.00, pread64[chat]=1.00, pread64[index.db]=1.55, pread64[ledger.db]=286.35, pread64[ledger.db-wal]=344.20, pwrite64[temp]=545.80, pwrite64[index.db-wal]=2.00, pwrite64[ledger.db]=295.90, pwrite64[ledger.db-wal]=641.25, statx[-]=1.00, statx[chat]=1.00, write[chat]=3.00 |
| 256-C | redact_recent | 4.95 | 43.50 | 74.90 | fdatasync[chat]=1.00, fstat[temp]=0.30, fstat[ledger.db]=1.00, fsync[ledger.db]=1.00, fsync[ledger.db-wal]=2.95, ftruncate[chat]=0.95, ftruncate[ledger.db]=1.00, ftruncate[ledger.db-wal]=1.00, newfstatat[-]=0.30, openat[-]=0.30, pread64[chat]=1.00, pread64[ledger.db]=2.45, pread64[ledger.db-wal]=40.05, pwrite64[temp]=0.70, pwrite64[index.db-wal]=1.90, pwrite64[ledger.db]=40.05, pwrite64[ledger.db-wal]=31.25, statx[-]=1.00, statx[chat]=1.00, write[chat]=1.00 |
| 256-C | unread_after_p | 0.00 | 3.15 | 0.00 | pread64[chat]=1.00, pread64[ledger.db]=2.15, statx[-]=1.00 |
