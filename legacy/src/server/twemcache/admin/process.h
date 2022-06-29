#pragma once

void admin_process_setup(void);
void admin_process_teardown(void);

void stats_dump(void *arg); /* compatible type: timeout_cb_fn */
