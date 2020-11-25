#include "request.h"

int LLVMFuzzerTestOneInput(const uint8_t *data, size_t size){

    // Null-terminate our string
    char *new_str = (char *)malloc(size+1);
    if (new_str == NULL){
            return 0;
    }
    memcpy(new_str, data, size);
    new_str[size] = '\0';

    // Create a constant
    char str[(int)size+1];
    snprintf(str, (int)size+1, "%s",  new_str);
    int len = sizeof(str);

    // Create necessary structs
    struct request *req;
    struct buf *buf;

    buf = buf_create();
    buf_write(buf, str, len);
    req = request_create();

    if(req==NULL){
        printf("req is null\n");
        buf_destroy(&buf);
        free(new_str);
        return 0;
    }

    parse_req(req, buf);

    request_destroy(&req);
    buf_destroy(&buf);
    free(new_str);
    return 0;
}
