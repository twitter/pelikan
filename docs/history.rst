Genesis
=======
The Twitter Cache team (which was part of Runtime Systems, and before that,
Infrastructure) started working on a fork of Memcached 1.4.4 in 2010. In 2011,
with the launch of Haplo (timeline cache), it also took over the maintenance
and improvement of Redis. We also developed Fatcache (SSD cache) and Slimcache
(small object cache). Until the introduction of Pelikan, all these cache
backends had their own codebases, even though there was a huge amount of
feature overlap, especially at lower parts of the stack.

The proliferation of codebases serving similar purposes introduced many
problems: we cannot easily provide feature synergy for users who need a
combination of features in different repos, a bug fix/improvement that affects
multiple repos need to be implemented multiple times, there aren't enough
resources to cover/maintain all the codebases and evolve them at the same time,
we've been in maintenance mode more often than we intended. At the mean time,
synthesis is perfectly plausible, as we will show in project overview.

A unified cache is therefore both beneficial and possible, and we went with it.
Project Broadbill was born, which later got split and renamed as ccommon (Cache
Common) and Pelikan. The former focuses on the functionalities characterized by
an RPC server, with robust production support, which isn't particularly cache-
specific; the latter is a direct replacement of all the cache backends that we
are using today, built on top of ccommon, and will serve as the only cache
backend as we move forward.
