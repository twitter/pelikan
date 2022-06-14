#pragma once

#include <cc_metric.h>

#define METRIC_PRINT_FMT "STAT %s %s\r\n"
#define METRIC_PRINT_LEN 64 /* > 5("STAT ") + 32 (name) + 20 (value) + CRLF */
#define METRIC_DESCRIBE_FMT "%33s %15s %s\r\n"
#define METRIC_DESCRIBE_LEN 120 /* 34 (name) + 16 (type) + 68 (description) + CRLF */
#define METRIC_END "END\r\n"
#define METRIC_END_LEN (sizeof(METRIC_END) - 1)

#define VERSION_PRINTED "VERSION " VERSION_STRING "\r\n"

size_t print_stats(char *buf, size_t cap, struct metric *metrics, unsigned int nmetric);
