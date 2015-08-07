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

#include "cc_shared.h"

#include <cc_debug.h>

static bool event_init = false;
event_metrics_st *event_metrics = NULL;

void
event_setup(event_metrics_st *metrics)
{
    log_info("set up the %s module", EVENT_MODULE_NAME);

    event_metrics = metrics;
    EVENT_METRIC_INIT(event_metrics);

    if (event_init) {
        log_warn("%s has already been setup, overwrite", EVENT_MODULE_NAME);
    }
    event_init = true;
}

void
event_teardown(void)
{
    log_info("tear down the %s module", EVENT_MODULE_NAME);

    if (!event_init) {
        log_warn("%s has never been setup", EVENT_MODULE_NAME);
    }
    event_metrics = NULL;
    event_init = false;
}
