#ifndef _BB_CORE_H_
#define _BB_CORE_H_

/* NOTE(yao): the core file should be where everything related to the
 * event driven stuff is handled, it is not a replacement of main()
 */
#include <cc_define.h>
#include <cc_stream.h>

rstatus_t core_read(struct stream *stream);
rstatus_t core_write(struct stream *stream);

#endif /* _BB_CORE_H_ */
