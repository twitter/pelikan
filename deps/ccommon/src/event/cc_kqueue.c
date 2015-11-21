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

#include <cc_event.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mm.h>

#include <inttypes.h>
#include <string.h>
#include <sys/event.h>
#include <sys/errno.h>
#include <unistd.h>

#include "cc_shared.h"

struct event_base {
    int           kq;           /* kernel event queue descriptor */

    struct kevent *change;      /* change[] - events we want to monitor */
    int           nchange;      /* # change */

    struct kevent *event;       /* event[] - events that were triggered */
    int           nevent;       /* # events */
    int           nreturned;    /* # events placed in event[] */
    int           nprocessed;   /* # events processed from event[] */

    event_cb_fn    cb;           /* event callback */
};

struct event_base *
event_base_create(int nevent, event_cb_fn cb)
{
    struct event_base *evb;
    int status, kq;
    struct kevent *change, *event;

    ASSERT(nevent > 0);

    kq = kqueue();
    if (kq < 0) {
        log_error("kqueue failed: %s", strerror(errno));
        return NULL;
    }

    change = (struct kevent *)cc_calloc(nevent, sizeof(*change));
    if (change == NULL) {
        status = close(kq);
        if (status < 0) {
            log_warn("close kqueue fd %d failed, ignored: %s", kq,
                    strerror(errno));
        }
        return NULL;
    }

    event = (struct kevent *)cc_calloc(nevent, sizeof(*event));
    if (event == NULL) {
        cc_free(change);
        status = close(kq);
        if (status < 0) {
            log_warn("close kqueue fd %d failed, ignored: %s", kq,
                    strerror(errno));
        }
        return NULL;
    }

    evb = (struct event_base *)cc_alloc(sizeof(*evb));
    if (evb == NULL) {
        cc_free(change);
        cc_free(event);
        status = close(kq);
        if (status < 0) {
            log_warn("close kqueue fd %d failed, ignored: %s", kq,
                    strerror(errno));
        }
        return NULL;
    }

    evb->kq = kq;
    evb->change = change;
    evb->nchange = 0;
    evb->event = event;
    evb->nevent = nevent;
    evb->nreturned = 0;
    evb->nprocessed = 0;
    evb->cb = cb;

    log_info("kqueue fd %d with nevent %d", evb->kq, evb->nevent);

    return evb;
}

void
event_base_destroy(struct event_base **evb)
{
    int status;
    struct event_base *e = *evb;

    if (e == NULL) {
        return;
    }

    ASSERT(e->kq > 0);

    cc_free(e->change);
    cc_free(e->event);

    status = close(e->kq);
    if (status < 0) {
        log_warn("close kq %d failed, ignored: %s", e->kq, strerror(errno));
    }
    e->kq = -1;

    cc_free(e);

    *evb = NULL;
}

int
event_add_read(struct event_base *evb, int fd, void *data)
{
    struct kevent *event;

    ASSERT(evb != NULL);
    ASSERT(evb->kq > 0);
    ASSERT(evb->nchange < evb->nevent);
    ASSERT(fd > 0);

    event = &evb->change[evb->nchange++];
    EV_SET(event, fd, EVFILT_READ, EV_ADD | EV_CLEAR, 0, 0, data);
    kevent(evb->kq, evb->change, evb->nchange, NULL, 0, NULL);

    log_verb("adding read event at %p, nchange %d", event, evb->nchange);

    evb->nchange = 0;
    INCR(event_metrics, event_read);

    return 0;
}

int
event_add_write(struct event_base *evb, int fd, void *data)
{
    struct kevent *event;

    ASSERT(evb != NULL);
    ASSERT(evb->kq > 0);
    ASSERT(evb->nchange < evb->nevent);
    ASSERT(fd > 0);

    event = &evb->change[evb->nchange++];
    EV_SET(event, fd, EVFILT_WRITE, EV_ADD | EV_CLEAR, 0, 0, data);
    kevent(evb->kq, evb->change, evb->nchange, NULL, 0, NULL);

    log_verb("adding write event at %p, nchange %d", event, evb->nchange);

    evb->nchange = 0;
    INCR(event_metrics, event_write);

    return 0;
}

int
event_register(struct event_base *evb, int fd, void *data)
{
    event_add_read(evb, fd, data);
    event_add_write(evb, fd, data);

    return 0;
}

int
event_deregister(struct event_base *evb, int fd)
{
    struct kevent *event;

    ASSERT(evb != NULL);
    ASSERT(evb->kq > 0);
    ASSERT(evb->nchange < evb->nevent);
    ASSERT(fd > 0);

    event = &evb->change[evb->nchange++];
    EV_SET(event, fd, EVFILT_READ, EV_DELETE, 0, 0, 0);
    event = &evb->change[evb->nchange++];
    EV_SET(event, fd, EVFILT_WRITE, EV_DELETE, 0, 0, 0);
    kevent(evb->kq, evb->change, evb->nchange, NULL, 0, NULL);
    evb->nchange = 0;

    return 0;
}

int
event_wait(struct event_base *evb, int timeout)
{
    int kq;
    struct timespec ts, *tsp;

    ASSERT(evb != NULL);

    kq = evb->kq;

    ASSERT(kq > 0);

    /* kevent should block indefinitely if timeout < 0 */
    if (timeout < 0) {
        tsp = NULL;
    } else {
        tsp = &ts;
        tsp->tv_sec = timeout / 1000LL;
        tsp->tv_nsec = (timeout % 1000LL) * 1000000LL;
    }

    for (;;) {
        /*
         * kevent() is used both to register new events with kqueue, and to
         * retrieve any pending events. Changes that should be applied to the
         * kqueue are given in the change[] and any returned events are placed
         * in event[], up to the maximum sized allowed by nevent. The number
         * of entries actually placed in event[] is returned by the kevent()
         * call and saved in nreturned.
         *
         * Events are registered with the system by the application via a
         * struct kevent, and an event is uniquely identified with the system
         * by a (kq, ident, filter) tuple. This means that there can be only
         * one (ident, filter) pair for a given kqueue.
         */
        evb->nreturned = kevent(kq, evb->change, evb->nchange, evb->event,
                                evb->nevent, tsp);
        INCR(event_metrics, event_loop);
        evb->nchange = 0;
        if (evb->nreturned > 0) {
            INCR_N(event_metrics, event_total, evb->nreturned);
            for (evb->nprocessed = 0; evb->nprocessed < evb->nreturned;
                evb->nprocessed++) {
                struct kevent *ev = &evb->event[evb->nprocessed];
                uint32_t events = 0;

                log_verb("kevent %04"PRIX32" with filter %"PRIX16" triggered "
                          "on ident %d", ev->flags, ev->filter, ev->ident);

                /*
                 * If an error occurs while processing an element of the
                 * change[] and there is enough room in the event[], then the
                 * event event will be placed in the eventlist with EV_ERROR
                 * set in flags and the system error(errno) in data.
                 */
                if (ev->flags & EV_ERROR) {
                   /*
                    * Error messages that can happen, when a delete fails.
                    *   EBADF happens when the file descriptor has been closed
                    *   ENOENT when the file descriptor was closed and then
                    *   reopened.
                    *   EINVAL for some reasons not understood; EINVAL
                    *   should not be returned ever; but FreeBSD does :-\
                    * An error is also indicated when a callback deletes an
                    * event we are still processing. In that case the data
                    * field is set to ENOENT.
                    */

                    if (ev->data != ENOMEM && ev->data != EFAULT &&
                            ev->data != EACCES && ev->data != EINVAL) {
                        continue;
                    }
                    events |= EVENT_ERR;
                }

                if (ev->filter == EVFILT_READ) {
                    events |= EVENT_READ;
                }

                if (ev->filter == EVFILT_WRITE) {
                    events |= EVENT_WRITE;
                }

                if (evb->cb != NULL && events != 0) {
                    evb->cb(ev->udata, events);
                }
            }

            log_verb("returned %d events from kqueue fd %d", evb->nreturned, kq);

            return evb->nreturned;
        }

        if (evb->nreturned == 0) {
            if (timeout == -1) {
               log_error("indefinite wait on kqueue fd %d with %d events "
                         "returned no events", kq, evb->nevent);

                return -1;
            }

            log_vverb("wait on kqueue fd %d with nevent %d timeout "
                         "%d returned no events", kq, evb->nevent, timeout);

            return 0;
        }

        if (errno == EINTR) {
            continue;
        }

        log_error("wait on kqueue fd %d with nevent %d and timeout %d failed: "
                  "%s", kq, evb->nevent, timeout, strerror(errno));

        return -1;
    }

    NOT_REACHED();
}
