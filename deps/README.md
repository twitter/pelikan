# deps

The deps directory contains all the dependencies shipped as source packages. The project level CMakeLists.txt contains additional dependencies.

All packages in this directory are tracked using git subtree. For more information, follow [this blog|http://blogs.atlassian.com/2013/05/alternatives-to-git-submodule-git-subtree/], and a slightly outdated reference [here|http://www.git-scm.com/book/en/v1/Git-Tools-Subtree-Merging]

## List of dependencies
* ccommon(libccommon): a library providing generic, low-level functionalities useful for writing a RPC service.

## ccommon

Setup upstream/remote
```bash
git remote add ccommon_remote https://git.twitter.biz/ccommon
```

The first time we merge ccommon into deps, the following command was executed.
```bash
git checkout master
git subtree add --prefix=deps/ccommon/ ccommon_remote master --squash
```

To see the difference between local and upstream/remote
```bash
git diff-tree -p ccommon_remote/master
```

To update ccommon with upstream/remote changes
```bash
git fetch ccommon_remote master
git subtree pull --prefix=deps/ccommon/ ccommon_remote master --squash
```

To update upstream/remote with local changes
```bash
git subtree push --prefix=deps/ccommon/ ccommon_remote master --squash
```
