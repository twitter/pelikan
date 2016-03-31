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

#include <cc_option.h>

#include <cc_debug.h>
#include <cc_log.h>
#include <cc_mm.h>

#include <ctype.h>
#include <errno.h>
#include <inttypes.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define OPTION_INFO_FMT "name: %-31s type: %-15s  current: %-20s ( default: %-20s )"
#define OPTION_DESCRIBE_FMT  "%-31s %-15s %-20s %s"

char * option_type_str[] = {
    "boolean",
    "unsigned int",
    "double",
    "string"
};

static rstatus_i
_option_parse_bool(struct option *opt, const char *val_str)
{
    rstatus_i status = CC_OK;

    if (strlen(val_str) == 3 && str3cmp(val_str, 'y', 'e', 's')) {
        opt->set = true;
        opt->val.vbool = true;
    } else if (strlen(val_str) == 2 && str2cmp(val_str, 'n', 'o')) {
        opt->set = true;
        opt->val.vbool = false;
    } else {
        log_stderr("unrecognized boolean option (valid values: 'yes' or 'no'), "
                   "value provided: '%s'", val_str);

        status = CC_ERROR;
    }

    return status;
}

/* returns true if op1's precedence > op2's precedence, false otherwise */
static inline bool
_option_comp_op_precedence(char op1, char op2)
{
    return ((op1 == '*' || op1 == '/') && (op2 == '+' || op2 == '-'));
}

/* Convert a traditional notation mathematical expression to reverse Polish
 * notation (RPN). This is done using Dijkstra's shunting-yard algorithm:
 *
 * While there are tokens left to be read:
 *  - Read a token
 *    1. If token is a number, add it to the end of the output str
 *    2. If the token is an operator o_1:
 *       a. While there is an operator token o_2 on the operator stack:
 *          - If o_2's precedence is > that of o_1, pop it off the stack and
 *            onto the output
 *          - Else, break
 *       b. Push o_1 onto the stack
 *    3. If the token is an open parenthesis, push it onto the operator stack
 *    4. If the token is a close parenthesis:
 *       a. Pop operators on the stack onto the output until open parenthesis
 *          is encountered
 *       b. Pop the open parenthesis from the stack, but not onto the output
 *       c. If the stack runs out, there is a mismatched parenthesis error
 *  - When there are no more tokens to be read:
 *    While there are operators on the operator stack:
 *       a. If the operator is a parenthesis, there is a mismatched parenthesis
 *          error
 *       b. Otherwise, pop the operator onto the output
 */
static rstatus_i
_option_convert_rpn(const char *val_str, char *rpn)
{
    const char *read_ptr = val_str;
    char op_stack[OPTLINE_MAXLEN];
    uint16_t op_stack_len = 0;

    while (*read_ptr) {
        switch (*read_ptr) {
        case '0':
        case '1':
        case '2':
        case '3':
        case '4':
        case '5':
        case '6':
        case '7':
        case '8':
        case '9':
            /* number, add it to the output and advance read_ptr */
            *(rpn++) = *(read_ptr++);
            break;
        case '+':
        case '-':
        case '*':
        case '/':
            /* operator */
            *(rpn++) = ' ';

            while (op_stack_len > 0 && op_stack[op_stack_len - 1] != '(' &&
                    _option_comp_op_precedence(op_stack[op_stack_len - 1],
                    *read_ptr)) {
                /* stack not empty and o_2 > o_1 (see above) */

                /* pop operator off stack */
                *(rpn++) = op_stack[--op_stack_len];
            }

            /* push operator onto stack, advance read_ptr */
            op_stack[op_stack_len++] = *(read_ptr++);

            break;
        case '(':
            /* push parenthesis onto stack */
            op_stack[op_stack_len++] = *(read_ptr++);

            break;
        case ')':
            /* Pop operators until ( is encountered */
            while (op_stack_len > 0 && op_stack[op_stack_len - 1] != '(') {
                *(rpn++) = ' ';
                *(rpn++) = op_stack[--op_stack_len];
            }

            if (op_stack_len == 0) {
                /* parenthesis mismatch */
                log_stderr("option load failed: parenthesis mismatch");
                return CC_ERROR;
            }

            ASSERT(op_stack[op_stack_len - 1] == '(');
            --op_stack_len;
            ++read_ptr;

            break;
        case ' ':
        case '\t':
            /* white space, ignore */
            ++read_ptr;
            break;
        default:
            /* unrecognized character */
            log_stderr("option load failed: unrecognized char %c in int "
                    "expression", *read_ptr);
            return CC_ERROR;
        }
    }

    /* Pop all operators on the stack */
    while (op_stack_len > 0) {
        if (op_stack[op_stack_len - 1] == '(') {
            /* mismatched parenthesis */
            log_stderr("option load failed: parenthesis mismatch");
            return CC_ERROR;
        }

        *(rpn++) = ' ';
        *(rpn++) = op_stack[--op_stack_len];
    }

    /* Null terminate */
    *rpn = '\0';
    return CC_OK;
}

/* Evaluate given reverse Polish notation expression:
 *
 * While there are tokens to be read:
 *  - Read token
 *    1. If token is a number, push onto the stack
 *    2. Else, token is an operator
 *       a. All valid operators take 2 operands
 *       b. If there are fewer than 2 operands on stack, the expression is
 *          erroneous
 *       c. Pop 2 values from the stack, evaluate the operator, then push the
 *          value back onto the stack
 *  - Look at the stack. If there is more than 1 value, too many values were
 *    provided
 */
static rstatus_i
_option_eval_rpn(char *rpn, uintmax_t *val)
{
    uintmax_t stack[OPTLINE_MAXLEN];
    uint16_t stack_len = 0;
    char *token;

    /* tokenize rpn */
    for (token = strtok(rpn, " "); token; token = strtok(NULL, " ")) {
        if (isdigit(token[0])) {
            /* token is number */
            stack[stack_len++] = atoll(token);
        } else {
            /* token is operand */
            uintmax_t first, second, result;

            if (stack_len < 2) {
                log_stderr("RPN expression %s malformed; not enough operands.",
                        rpn);
                return CC_ERROR;
            }

            second = stack[--stack_len];
            first = stack[--stack_len];

            switch (token[0]) {
            case '+':
                result = first + second;

                if (result < first) {
                    /* integer overflow */
                    log_stderr("evaluating integer expression causes overflow");
                    return CC_ERROR;
                }

                break;
            case '-':
                result = first - second;

                if (result > first) {
                    /* subtraction causes op2 to be negative */
                    log_stderr("unsigned integer expression contains negative "
                            "number");
                    return CC_ERROR;
                }

                break;
            case '*':
                result = first * second;

                if (first != 0 && result / first != second) {
                    /* overflow */
                    log_stderr("evaluating integer expression causes overflow");
                    return CC_ERROR;
                }

                break;
            case '/':
                if (second == 0) {
                    /* divide by zero */
                    log_stderr("evaluating integer expression causes divide by "
                            "zero");
                    return CC_ERROR;
                }

                result = first / second;
                break;
            default:
                NOT_REACHED();
                result = 0;
            }

            stack[stack_len++] = result;
        }
    }

    if (stack_len != 1) {
        log_stderr("RPN expression %s malformed; too many operands.", rpn);
        return CC_ERROR;
    }

    *val = stack[0];
    return CC_OK;
}

/* Evaluate integer expression. We do this by taking the following steps:
 *  1. convert val_str to reverse Polish notation (RPN)
 *  2. evaluate RPN expression
 */
static rstatus_i
_option_eval_int_expr(const char *val_str, uintmax_t *val)
{
    char rpn[OPTLINE_MAXLEN];
    rstatus_i ret;

    ASSERT(val_str != NULL);
    ASSERT(val != NULL);

    ret = _option_convert_rpn(val_str, rpn);

    if (ret != CC_OK) {
        log_stderr("invalid integer expression %s", val_str);
        return ret;
    }

    ret = _option_eval_rpn(rpn, val);
    return ret;
}

static rstatus_i
_option_parse_uint(struct option *opt, const char *val_str)
{
    uintmax_t val = 0;

    if (_option_eval_int_expr(val_str, &val) != CC_OK) {
        log_stderr("option value %s could not be parsed as an integer "
                "expression", val_str);
        return CC_ERROR;
    }

    opt->set = true;
    opt->val.vuint = val;

    return CC_OK;
}

static rstatus_i
_option_parse_fpn(struct option *opt, const char *val_str)
{
    /* TODO: handle expressions similar to what's allowed with integers */
    double val = 0;
    char *loc;

    val = strtod(val_str, &loc);
    if (errno == ERANGE) {
        log_stderr("option value %s out of range for double type", val_str);
        return CC_ERROR;
    }

    if (*loc != '\0') {
        log_stderr("option value %s could not be fully parsed, check char at "
                "offset %ld", val_str, loc - val_str);
        return CC_ERROR;
    }

    opt->set = true;
    opt->val.vfpn = val;

    return CC_OK;
}

static rstatus_i
_option_parse_str(struct option *opt, const char *val_str)
{
    opt->set = true;
    if (opt->val.vstr) {
        cc_free(opt->val.vstr);
    }

    if (val_str == NULL) {
        opt->val.vstr = NULL;
        return CC_OK;
    }

    opt->val.vstr = cc_alloc(strlen(val_str) + 1);
    if (opt->val.vstr == NULL) {
        log_crit("cannot store configuration string, OOM");
        return CC_ERROR;
    }
    strcpy(opt->val.vstr, val_str);

    return CC_OK;
}

rstatus_i
option_default(struct option *opt)
{
    opt->set = true;
    switch (opt->type) {
    case OPTION_TYPE_BOOL:
        opt->val.vbool = opt->default_val.vbool;
        break;

    case OPTION_TYPE_UINT:
        opt->val.vuint = opt->default_val.vuint;
        break;

    case OPTION_TYPE_FPN:
        opt->val.vfpn = opt->default_val.vfpn;
        break;

    case OPTION_TYPE_STR:
        if (opt->default_val.vstr == NULL) {
            opt->val.vstr = NULL;
            return CC_OK;
        }
        opt->val.vstr = cc_alloc(strlen(opt->default_val.vstr) + 1);
        if (opt->val.vstr == NULL) {
            log_crit("cannot store configuration string, OOM");
            return CC_ERROR;
        }
        strcpy(opt->val.vstr, opt->default_val.vstr);
        break;

    default:
        opt->set = false;
        log_stderr("option set error: unrecognized option type");
        return CC_ERROR;
    }

    return CC_OK;
}

rstatus_i
option_set(struct option *opt, char *val_str)
{
    switch (opt->type) {
    case OPTION_TYPE_BOOL:
        return _option_parse_bool(opt, val_str);

    case OPTION_TYPE_UINT:
        return _option_parse_uint(opt, val_str);

    case OPTION_TYPE_FPN:
        return _option_parse_fpn(opt, val_str);

    case OPTION_TYPE_STR:
        return _option_parse_str(opt, val_str);

    default:
        log_stderr("option set error: unrecognized option type");
        return CC_ERROR;
    }

    NOT_REACHED();
}

static inline bool
_allowed_in_name(char c)
{
    /* the criteria is C's rules on variable names since we use it as such */
    if ((c >= 'a' && c <= 'z') || c == '_' || (c >= 'A' && c <= 'Z') ||
        (c >= '0' && c <= '9')) {
        return true;
    } else {
        return false;
    }
}

static rstatus_i
_option_parse(char *line, char name[OPTNAME_MAXLEN+1], char val[OPTVAL_MAXLEN+1])
{
    char *p = line;
    char *q;
    size_t vlen, llen = strlen(line);

    if (strlen(line) == 0 || isspace(line[0]) || line[0] == '#') {
        return CC_EEMPTY;
    }

    if (llen > OPTLINE_MAXLEN) {
        log_stderr("option parse error: line length %zu exceeds limit %zu",
                   llen, OPTLINE_MAXLEN);

        return CC_ERROR;
    }

    /* parse name */
    while (*p != ':' && (size_t)(p - line) < MIN(llen, OPTNAME_MAXLEN + 1)) {
        if (_allowed_in_name(*p)) {
            *name = *p;
            name++;
        } else {
            log_stderr("option parse error: invalid char'%c' at pos %d in name",
                       *p, (p - line));

            return CC_ERROR;
        }
        p++;
    }
    if ((size_t)(p - line) == llen) {
        log_stderr("option parse error: incomplete option line");

        return CC_ERROR;
    }
    if ((size_t)(p - line) > OPTNAME_MAXLEN) {
        log_stderr("option parse error: name too long (max %zu)",
                   OPTNAME_MAXLEN);

        return CC_ERROR;
    }
    *name = '\0'; /* terminate name string properly */

    /* parse value: l/rtrim WS characters */
    p++; /* move over ':' */
    q = line + llen - 1;
    while (isspace(*p) && p < q) {
        p++;
    }
    while (isspace(*q) && q >= p) {
        q--;
    }
    if (p > q) {
        log_stderr("option parse error: empty value");

        return CC_ERROR;
    }
    vlen = q - p + 1; /* +1 because value range is [p, q] */
    if (vlen > OPTVAL_MAXLEN) {
        log_stderr("option parse error: value too long (max %zu)",
                   OPTVAL_MAXLEN);

        return CC_ERROR;
    }
    /*
     * Here we don't use strlcpy() below because value is not NULL-terminated.
     * As long as the buffers parsed in are big enough (satisfy ASSERTs above),
     * we should be fine.
     */
    strncpy(val, p, vlen);
    *(val + vlen) = '\0'; /* terminate value string properly */

    return CC_OK;
}

static void
option_print_val(char *s, size_t len, option_type_e type, option_val_u val)
{
    switch (type) {
    case OPTION_TYPE_BOOL:
        snprintf(s, len, "%s", val.vbool ? "yes" : "no");
        break;

    case OPTION_TYPE_UINT:
        snprintf(s, len, "%ju", val.vuint);
        break;

    case OPTION_TYPE_FPN:
        snprintf(s, len, "%f", val.vfpn);
        break;

    case OPTION_TYPE_STR:
        snprintf(s, len, "%s", val.vstr == NULL ? "NULL" : val.vstr);
        break;

    default:
        NOT_REACHED();
    }
}

void
option_print(struct option *opt)
{
    char default_s[PATH_MAX];
    char current_s[PATH_MAX];

    option_print_val(default_s, PATH_MAX, opt->type, opt->default_val);
    option_print_val(current_s, PATH_MAX, opt->type, opt->val);
    log_stdout(OPTION_INFO_FMT, opt->name, option_type_str[opt->type],
            current_s, default_s);
}

void
option_print_all(struct option options[], unsigned int nopt)
{
    unsigned int i;
    struct option *opt = options;

    for (i = 0; i < nopt; i++, opt++) {
        option_print(opt);
    }

}

static void
_option_describe(struct option *opt)
{
    char default_s[PATH_MAX + 10];

    option_print_val(default_s, PATH_MAX + 10, opt->type, opt->default_val);
    log_stdout(OPTION_DESCRIBE_FMT, opt->name, option_type_str[opt->type],
            default_s, opt->description);
}

void
option_describe_all(struct option options[], unsigned int nopt)
{
    unsigned int i;

    /* print a header */
    log_stdout(OPTION_DESCRIBE_FMT, "NAME", "TYPE", "DEFAULT", "DESCRIPTION");

    for (i = 0; i < nopt; i++, options++) {
        _option_describe(options);
    }
}

rstatus_i
option_load_default(struct option options[], unsigned int nopt)
{
    unsigned int i;
    rstatus_i status;

    for (i = 0; i < nopt; i++) {
        status = option_default(&options[i]);
        if (status != CC_OK) {
            return status;
        }
    }

    return CC_OK;
}

rstatus_i
option_load_file(FILE *fp, struct option options[], unsigned int nopt)
{
    /* Note: when in use, all bufs are '\0' terminated if no error occurs */
    char linebuf[OPTLINE_MAXLEN + 1];
    char namebuf[OPTNAME_MAXLEN + 1];
    char valbuf[OPTVAL_MAXLEN + 1];
    rstatus_i status;
    struct option *opt;
    bool match;
    unsigned int i;
    int fe;

    while (fgets(linebuf, OPTLINE_MAXLEN + 1, fp) != NULL) {
        status = _option_parse(linebuf, namebuf, valbuf);
        if (status == CC_EEMPTY) {
            continue;
        }
        if (status != CC_OK) {
            log_stderr("error loading config line %s", linebuf);

            return CC_ERROR;
        }

        opt = options;
        match = false;
        for (i = 0; i < nopt; i++, opt++) {
            if (cc_strcmp(namebuf, opt->name) == 0) {
                match = true;
                status = option_set(opt, valbuf);
                break;
            }
        }
        if (!match) {
            log_stderr("error loading config line: no option named '%s'",
                       namebuf);

            return CC_ERROR;
        }
        if (status != CC_OK) {
            log_stderr("error applying value '%s' to option '%s': error %d.",
                       valbuf, namebuf, status);

            return CC_ERROR;
        }
    }

    fe = ferror(fp);
    if (fe != 0) {
        log_stderr("load config failed due to file error: %d", fe);

        return CC_ERROR;
    }

    return CC_OK;
}

void
option_free(struct option options[], unsigned int nopt)
{
    unsigned int i;
    struct option *opt = options;

    for (i = 0; i < nopt; ++i, ++opt) {
        if (opt->type == OPTION_TYPE_STR && opt->val.vstr != NULL) {
            cc_free(opt->val.vstr);
        }
    }
}
