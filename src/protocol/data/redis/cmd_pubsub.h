#pragma once

/*          type                string          # of args */
#define REQ_PUBSUB(ACTION)                          \
    ACTION( REQ_PUBLISH,        "publish",      3  )\
    ACTION( REQ_SUBSCRIBE,      "subscribe",    -2 )\
    ACTION( REQ_UNSUBSCRIBE,    "unsubscribe",  -2 )
