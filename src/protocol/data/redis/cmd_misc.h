#pragma once

/*          type            string      # of args */
#define REQ_MISC(ACTION)                    \
    ACTION( REQ_FLUSHALL,   "flushall", 1  )\
    ACTION( REQ_PING,       "ping",     -1 )\
    ACTION( REQ_QUIT,       "quit",     1  )
