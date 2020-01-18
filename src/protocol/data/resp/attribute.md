# Use of attributes

## Background

Redis [RESP 3 spec](https://github.com/antirez/RESP3/blob/master/spec.md)
proposes a slew of new types, including attributes (optional key-value pairs).
The original proposal only allows them to be used in responses, but they can
be nested anywhere in the message.

## Use Cases

We are interested in a few use cases that are related to attributes, such as:
 - setting new TTLs with updates or even read access to improve eviction
 behavior;
 - storing "true age" of a key (instead of when it was stuck into cache) to
 inform client on best actions against the value (e.g. more recent keys may
 see more activities);
 - signaling notable event on the key back to the caller, such as a hot key.

## Modifications

### Allow Attributes in Both `Request` and `Response`

To support these use cases, and maybe more future ones, it is necessary to
allow attributes to be present in both requests and responses. For example,
one might want to extend the TTL whenever they are appending to a list, without
having to issue another two commands, `EXPIRE` and `TTL`, as they would need to
in current Redis.

### Limited Global Attributes

The parser is opinionated about which top-level attributes are allowed via a
preset list in `attribute.h`, with related support in request and response
object declareations. This is due to the fact that attributes can be nested
anywhere, and therefore more local options should be declared, and therefore
handled, in a more local fashion. In exchange, knowing what attributes to
expect allows us to handle them with less code in the protocol module. Of
course, it is easy to expand this list for legitimate use cases.

Another limitation on global attributes is that we expect the keys to be simple
strings, while the values should be integers. Obviously this assumption could
change with the right use case. But for now, that covers all we want and is very
easy to code up.

## Notes on Other Usage

With attributes, it might make sense to convert some optional command arguments
to use attributes, or combine some commands that are highly similar with small
variants, or provide new flexibility into how the command should behave. For
example, there are obviously multiple ways to respond to an operation that
modifies a complex data structure with a small delta, such as simply acknowledge
the status of the execution, returning a summary or metadata about the key, or
responding with the whole value after modification. This has historically been
difficult to predetermine which response is the best, and can be highly use-case
dependent. With attributes, it would be easy to declare a reply style within the
request itself, or request any number of metadata to be included in the response
as attributes along side the main value(s).
