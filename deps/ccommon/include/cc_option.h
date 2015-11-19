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

#pragma once

#ifdef __cplusplus
extern "C" {
#endif

#include <cc_define.h>
#include <cc_bstring.h>
#include <cc_util.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

#define OPTLINE_MAXLEN  1024
#define OPTNAME_MAXLEN  31
#define OPTVAL_MAXLEN   255

/*
 * Each option is described by a 4-tuple:
 *      (NAME, TYPE, DEFAULT, DESCRIPTION)
 *   - NAME has to be a legal C variable name
 *   - TYPE supported types include: boolean, int, float, string
 *   - DEFAULT is the default value of the option, as a string
 *   - DESCRIPTION is a brief description of what the option does.
 */

/* TODO(yao): add an (optional) callback that can sanity-check input values */


#define OPTION_DECLARE(_name, _type, _default, _description)                \
    struct option _name;

/* Initialize option */
#define OPTION_INIT(_name, _type, _default, _description)                   \
    ._name = {.name = #_name, .set = false, .type = _type,                  \
        .default_val_str = _default, .description = _description},

#define OPTION_CARDINALITY(_o) sizeof(_o)/sizeof(struct option)

/* Enum used to match setting to type in order to set values */
typedef enum option_type {
    OPTION_TYPE_BOOL,
    OPTION_TYPE_UINT,
    OPTION_TYPE_STR,
    OPTION_TYPE_SENTINEL
} option_type_e;
extern char *option_type_str[];

/* Union containing payload for setting */
typedef union option_val {
    bool vbool;
    uintmax_t vuint;
    char *vstr;
} option_val_u;

/* Struct containing data for one individual setting */
struct option {
    char *name;
    bool set;
    option_type_e type;
    char *default_val_str;
    option_val_u val;
    char *description;
};

rstatus_i option_set(struct option *opt, char *val_str);
rstatus_i option_parse(char *line, char *name, char *val);
void option_print(struct option *opt);
void option_printall(struct option options[], unsigned int nopt);
rstatus_i option_load_default(struct option options[], unsigned int nopt);
rstatus_i option_load_file(FILE *fp, struct option options[], unsigned int nopt);
void option_free(struct option options[], unsigned int nopt);

#ifdef __cplusplus
}
#endif
