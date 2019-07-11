## Examples

Below are some examples where cluster sizing is driven by different factors.
Note that `twemcache` asks for _average_ key+val size in bytes via `--size` option,
while `slimcache` asks for size of _largest_ key+value in bytes via `--size`
option. They are otherwise similar.

Twemcache Examples:

```sh
# throughput driven
python3 calculator.py twemcache --qps 1000 --size 200 --nkey 500 --nconn 2000 --failure_domain 5

# memory driven
python3 calculator.py twemcache --qps 1000 --size 1000 --nkey 500 --nconn 2000 --failure_domain 5
python3 calculator.py twemcache --qps 1000 --size 400 --nkey 500 --nconn 2000 --failure_domain 5

# failure domain driven
python3 calculator.py twemcache --qps 1000 --size 200 --nkey 500 --nconn 2000 --failure_domain 0.5
```

Slimcache Examples:

```sh
# throughput driven
python3 calculator.py slimcache --qps 1000 --size 45 --nkey 500 --nconn 2000 --failure_domain 5

# memory driven
python3 calculator.py slimcache --qps 1000 --size 45 --nkey 4000 --nconn 2000 --failure_domain 5
python3 calculator.py slimcache --qps 1000 --size 80 --nkey 5000 --nconn 2000 --failure_domain 5

# failure domain driven
python3 calculator.py slimcache --qps 1000 --size 48 --nkey 500 --nconn 2000 --failure_domain 0.5
```

```sh
python3 calculator.py twemcache -h
python3 calculator.py slimcache -h
```

