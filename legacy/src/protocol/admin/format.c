#include "format.h"

#include <cc_print.h>

size_t
print_stats(char *buf, size_t cap, struct metric *metrics, unsigned int nmetric)
{
    size_t offset = 0;

    /* TODO: report error if running out of space in buf */
    for (int i = 0; i < nmetric; ++i) {
        offset += metric_print(buf + offset, cap - offset, METRIC_PRINT_FMT,
                &metrics[i]);
    }
    offset += cc_scnprintf(buf + offset, cap - offset, METRIC_END);

    return offset;
}
