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
#include <cc_print.h>

#include <stdbool.h>

#define METRIC_MODULE_NAME "ccommon::metric"

static bool metric_init = false;

void
metric_reset(struct metric sarr[], unsigned int n)
{
    unsigned int i;
    for (i = 0; i < n; i++) {
        switch (sarr[i].type) {
        case METRIC_COUNTER:
            sarr[i].counter = 0;
            break;

        case METRIC_GAUGE:
            sarr[i].gauge = 0;
            break;

        case METRIC_DDOUBLE:
            sarr[i].vdouble = 0.0;
            break;

        case METRIC_DINTMAX:
            sarr[i].vintmax = 0;
            break;

        default:
            NOT_REACHED();
            break;
        }
    }
}

void
metric_setup(void)
{
    log_info("set up the %s module", METRIC_MODULE_NAME);

    if (metric_init) {
        log_warn("%s has already been setup, overwrite", METRIC_MODULE_NAME);
    }
    metric_init = true;
}

void
metric_teardown(void)
{
    log_info("tear down the %s module", METRIC_MODULE_NAME);

    if (!metric_init) {
        log_warn("%s has never been setup", METRIC_MODULE_NAME);
    }
    metric_init = false;
}

size_t
metric_print(char *buf, size_t nbuf, struct metric *m)
{
    switch(m->type) {
    case METRIC_COUNTER:
        /**
         * not using cc_print_uint64, since it would complicate implementation
         * and negatively impact readability, and since this function should not
         * be called often enough to make it absolutely performance critical.
         */
        return cc_scnprintf(buf, nbuf, "%s %llu", m->name, __atomic_load_n(&m->counter, __ATOMIC_RELAXED));
    case METRIC_GAUGE:
        return cc_scnprintf(buf, nbuf, "%s %lld", m->name, __atomic_load_n(&m->gauge, __ATOMIC_RELAXED));
    case METRIC_DDOUBLE:
        return cc_scnprintf(buf, nbuf, "%s %f", m->name, m->vdouble);
    case METRIC_DINTMAX:
        return cc_scnprintf(buf, nbuf, "%s %lld", m->name, m->vintmax);
    default:
        NOT_REACHED();
        return 0;
    }
}
