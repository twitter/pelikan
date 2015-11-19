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

/* need the following to use EPOLLRDHUP
 * #define _GNU_SOURCE */
#include <cc_event.h>

#include <cc_debug.h>
#include <cc_define.h>
#include <cc_mm.h>

#include <inttypes.h>
#include <string.h>
#include <sys/epoll.h>
#include <sys/errno.h>
#include <unistd.h>

#include "cc_shared.h"

/*
 * glibc added EPOLLRDHUP much later than the kernel support,
 * so we may need to define it ourselves
 */
#ifndef EPOLLRDHUP
# define EPOLLRDHUP 0x2000
#endif

struct event_base {
    int                ep;      /* epoll descriptor */

    struct epoll_event *event;  /* event[] - events that were triggered */
    int                nevent;  /* # events */

    event_cb_fn         cb;      /* event callback */
};

struct event_base *
event_base_create(int nevent, event_cb_fn cb)
{
    struct event_base *evb;
    int status, ep;
    struct epoll_event *event;

    ASSERT(nevent > 0);

    ep = epoll_create(nevent);
    if (ep < 0) {
        log_error("epoll create size %d failed: %s", nevent, strerror(errno));
        return NULL;
    }

    event = (struct epoll_event *)cc_calloc(nevent, sizeof(*event));
    if (event == NULL) {
        status = close(ep);
        if (status < 0) {
            log_warn("close e %d failed, ignored: %s", ep, strerror(errno));
        }
        return NULL;
    }

    evb = (struct event_base *)cc_alloc(sizeof(*evb));
    if (evb == NULL) {
        cc_free(event);
        status = close(ep);
        if (status < 0) {
            log_warn("close e %d failed, ignored: %s", ep, strerror(errno));
        }
        return NULL;
    }

    evb->ep = ep;
    evb->event = event;
    evb->nevent = nevent;
    evb->cb = cb;

    log_info("epoll fd %d with nevent %d", evb->ep, evb->nevent);

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

    ASSERT(e->ep > 0);

    cc_free(e->event);

    status = close(e->ep);
    if (status < 0) {
        log_warn("close e %d failed, ignored: %s", e->ep, strerror(errno));
    }
    e->ep = -1;

    cc_free(e);

    *evb = NULL;
}

int event_add_read(struct event_base *evb, int fd, void *data)
{
    int status;
    struct epoll_event event;
    int ep;

    ASSERT(evb != NULL);

    ep = evb->ep;

    ASSERT(ep > 0);
    ASSERT (fd > 0);

    event.events = EPOLLIN;
    //event.events = (EPOLLIN | EPOLLET);
    event.data.ptr = data;

    /*
     * Note(yao): there have been tests showing EPOLL_CTL_ADD is cheaper than
     * EPOLL_CTL_MOD, and the only difference is we need to ignore EEXIST
     */
    status = epoll_ctl(ep, EPOLL_CTL_ADD, fd, &event);
    if (status < 0 && errno != EEXIST) {
        log_error("ctl (add read) w/ epoll fd %d on fd %d failed: %s", ep, fd,
                strerror(errno));
    }

    INCR(event_metrics, event_read);
    log_verb("add read event to epoll fd %d on fd %d", ep, fd);

    return status;
}

int
event_add_write(struct event_base *evb, int fd, void *data)
{
    int status;
    struct epoll_event event;
    int ep;

    ASSERT(evb != NULL);

    ep = evb->ep;

    ASSERT(ep > 0);
    ASSERT(fd > 0);

    event.events = EPOLLOUT;
    //event.events = (EPOLLOUT | EPOLLET);
    event.data.ptr = data;

    /* Note(yao): see note in event_add_read about epoll_ctl() */
    status = epoll_ctl(ep, EPOLL_CTL_ADD, fd, &event);
    if (status < 0 && errno != EEXIST) {
        log_error("ctl (add write) w/ epoll fd %d on fd %d failed: %s", ep, fd,
                status, strerror(errno));
    }

    INCR(event_metrics, event_write);
    log_verb("add write event to epoll fd %d on fd %d", ep, fd);

    return status;
}

int
event_register(struct event_base *evb, int fd, void *data)
{
    int status;
    struct epoll_event event;
    int ep;

    ASSERT(evb != NULL);

    ep = evb->ep;

    ASSERT(ep > 0);
    ASSERT(fd > 0);

    event.events = (EPOLLOUT | EPOLLIN);
    //event.events = (EPOLLOUT | EPOLLIN | EPOLLET);
    event.data.ptr = data;

    status = epoll_ctl(ep, EPOLL_CTL_ADD, fd, &event);
    if (status < 0) {
        log_error("ctl (reg) of fd %d to epoll fd %d failed: %s", fd, ep,
                strerror(errno));
    }

    log_verb("register fd %d to epoll fd %d", fd, ep);

    return status;
}

int
event_deregister(struct event_base *evb, int fd)
{
    int status;
    struct epoll_event event;
    int ep;

    ASSERT(evb != NULL);

    ep = evb->ep;

    ASSERT(ep > 0);
    ASSERT(fd > 0);

    /* event can be NULL in kernel >=2.6.9, here we keep it for compatibility */
    status = epoll_ctl(ep, EPOLL_CTL_DEL, fd, &event);
    if (status < 0) {
        log_error("ctl (dereg) of fd %d from epoll fd %d failed: %s", fd, ep,
                strerror(errno));
    }

    log_verb("deregister fd %d from epoll fd %d", fd, ep);

    return status;
}


/*
 * create a timed event with event base function and timeout (in millisecond)
 */
int
event_wait(struct event_base *evb, int timeout)
{
    struct epoll_event *ev_arr;
    int nevent;
    int ep;

    ASSERT(evb != NULL);

    ep = evb->ep;
    ev_arr = evb->event;
    nevent = evb->nevent;

    ASSERT(ep > 0);
    ASSERT(ev_arr != NULL);
    ASSERT(nevent > 0);

    for (;;) {
        int i, nreturned;

        nreturned = epoll_wait(ep, ev_arr, nevent, timeout);
        INCR(event_metrics, event_loop);
        if (nreturned > 0) {
            INCR_N(event_metrics, event_total, nreturned);
            for (i = 0; i < nreturned; i++) {
                struct epoll_event *ev = ev_arr + i;
                uint32_t events = 0;

                log_verb("epoll %04"PRIX32" against data %p",
                          ev->events, ev->data.ptr);


                if (ev->events & (EPOLLERR | EPOLLHUP)) {
                    events |= EVENT_ERR;
                }

                if (ev->events & (EPOLLIN | EPOLLRDHUP)) {
                    events |= EVENT_READ;
                }

                if (ev->events & EPOLLOUT) {
                    events |= EVENT_WRITE;
                }

                if (evb->cb != NULL) {
                    evb->cb(ev->data.ptr, events);
                }
            }

            log_verb("returned %d events from epoll fd %d",
                    nreturned, ep);

            return nreturned;
        }

        if (nreturned == 0) {
            if (timeout == -1) {
               log_error("indefinite wait on epoll fd %d with %d events "
                         "returned no events", ep, nevent);
                return -1;
            }

            log_vverb("wait on epoll fd %d with nevent %d timeout %d"
                         "returned no events", ep, nevent, timeout);
            return 0;
        }

        if (errno == EINTR) {
            continue;
        }

        log_error("wait on epoll fd %d with nevent %d and timeout %d failed: "
                "%s", ep, nevent, strerror(errno));

        return -1;
    }

    NOT_REACHED();
}
