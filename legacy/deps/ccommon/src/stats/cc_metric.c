/*
 * ccommon - a cache common library.
 * Copyright (C) 2013 Twitter, Inc.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 * http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

#include <cc_metric.h>

#include <cc_debug.h>
#include <cc_log.h>
#include <cc_print.h>

#include <stdbool.h>

#define VALUE_PRINT_LEN 30
#define METRIC_DESCRIBE_FMT  "%-31s %-15s %s"

char *metric_type_str[] = {"counter", "gauge", "floating point"};

void
metric_reset(struct metric sarr[], unsigned int n)
{
    unsigned int i;

    if (sarr == NULL) {
        return;
    }

    for (i = 0; i < n; i++) {
        switch (sarr[i].type) {
        case METRIC_COUNTER:
            sarr[i].counter = 0;
            break;

        case METRIC_GAUGE:
            sarr[i].gauge = 0;
            break;

        case METRIC_FPN:
            sarr[i].fpn = 0.0;
            break;

        default:
            NOT_REACHED();
            break;
        }
    }
}

size_t
metric_print(char *buf, size_t nbuf, char *fmt, struct metric *m)
{
    char val_buf[VALUE_PRINT_LEN];

    if (m == NULL) {
        return 0;
    }

    switch(m->type) {
    case METRIC_COUNTER:
        /**
         * not using cc_print_uint64, since it would complicate implementation
         * and negatively impact readability, and since this function should not
         * be called often enough to make it absolutely performance critical.
         */
        cc_scnprintf(val_buf, VALUE_PRINT_LEN, "%llu", __atomic_load_n(
                    &m->counter, __ATOMIC_RELAXED));
        break;

    case METRIC_GAUGE:
        cc_scnprintf(val_buf, VALUE_PRINT_LEN, "%lld", __atomic_load_n(
                    &m->gauge, __ATOMIC_RELAXED));
        break;

    case METRIC_FPN:
        cc_scnprintf(val_buf, VALUE_PRINT_LEN, "%f", m->fpn);
        break;

    default:
        NOT_REACHED();
    }

    return cc_scnprintf(buf, nbuf, fmt, m->name, val_buf);
}

void
metric_describe_all(struct metric metrics[], unsigned int nmetric)
{
    unsigned int i;

    /* print a header */
    log_stdout(METRIC_DESCRIBE_FMT, "NAME", "TYPE", "DESCRIPTION");

    for (i = 0; i < nmetric; i++, metrics++) {
        log_stdout(METRIC_DESCRIBE_FMT, metrics->name,
                metric_type_str[metrics->type], metrics->desc);
    }
}
