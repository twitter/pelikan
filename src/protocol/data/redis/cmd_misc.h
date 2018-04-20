#pragma once

/*          type            string      #arg#opt */
#define REQ_MISC(ACTION)                        \
    ACTION( REQ_FLUSHALL,   "flushall", 1,  0  )\
    ACTION( REQ_PING,       "ping",     1,  1  )\
    ACTION( REQ_QUIT,       "quit",     1,  0  )
