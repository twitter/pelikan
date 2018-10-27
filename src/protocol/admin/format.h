#pragma once

#include <cc_metric.h>

#define METRIC_PRINT_FMT "STAT %s %s\r\n"
#define METRIC_PRINT_LEN 64 /* > 5("STAT ") + 32 (name) + 20 (value) + CRLF */
#define METRIC_DESCRIBE_FMT "%33s %15s %s\r\n"
#define METRIC_DESCRIBE_LEN 120 /* 34 (name) + 16 (type) + 68 (description) + CRLF */
#define METRIC_END "END\r\n"
#define METRIC_END_LEN (sizeof(METRIC_END) - 1)
#define CENSUS_COUNT_FMT "item count: %zu %zu %zu\r\n"
#define CENSUS_COUNT_LEN 34 /* 12 (name string) + 20 + CRLF */
#define CENSUS_KEY_FMT "key min: %zu, max: %zu, total: %zu\r\n"
#define CENSUS_KEY_LEN 87 /* 9 + 7 + 9 (name strings) + 20 * 3 + CRLF */
#define CENSUS_VAL_FMT "val min: %zu, max: %zu, total: %zu\r\n"
#define CENSUS_VAL_LEN 87 /* 9 + 7 + 9 (name strings) + 20 * 3 + CRLF */
#define CENSUS_FMT CENSUS_COUNT_FMT CENSUS_KEY_FMT CENSUS_VAL_FMT
#define CENSUS_LEN CENSUS_COUNT_LEN + CENSUS_KEY_LEN + CENSUS_VAL_LEN

#define VERSION_PRINTED "VERSION " VERSION_STRING "\r\n"

size_t print_stats(char *buf, size_t cap, struct metric *metrics, unsigned int nmetric);
