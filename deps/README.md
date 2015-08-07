# deps

The deps directory contains all the dependencies shipped as source packages. The project level CMakeLists.txt contains additional dependencies.

All packages in this directory are tracked using git subtree. For more information, follow the original [git-subtree documentation](https://github.com/apenwarr/git-subtree/blob/ master/git-subtree.txt)

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

To update ccommon with upstream/remote changes
```bash
git fetch ccommon_remote master
git subtree pull --prefix=deps/ccommon/ --squash ccommon_remote master
```

To update upstream/remote with local changes involves somewhat complicated commands. At a high level, this is done in two steps: first, the local history needs to be sifted to i solate changes that are relevant to the subtree (deps/ccommon in our case), and an alternative "timeline" or history suitable for committing to the remote is created; second, thi s alternative history is pushed back to the remote. See Notes and subtree's github repo for more information.
```bash
# first find out the last SHA of a merge from upstream, in this example it is a06437
git subtree split --prefix=deps/ccommon --annotate='ccommon: ' a064371781e7fa4be044b80353dde9014353d6a5^.. -b ccommon_update
git push ccommon_remote ccommon_update:master
# -b is optional- it is perfectly fine to push without a branch.
# The split command return a SHA, if -b is not present, and the SHA value can be used in place of the branch name.
```

# Notes

## Why subtree?
There are two goals we try to achieve: a) manage dependencies explicitly; b) make the repo easy to use for most people, not just main developers.
The dependency management went through three phases: no management at all (two free standing repos); use of git submodule; use of git subtree. It was very clear that no dependency management was absolutely unacceptable, build broke all the time. Git submodule is the first thing we tried, and it is fairly easy to make changes within the submodule and merge the changes back to upstream. However, since submodules require extra options to be fully checked out or updated, it becomes harder to use especially for people unfamiliar with the project.
Git subtree actually means two things: the subtree mechanism that manages sub-repository using native git commands, and the git subtree extension, which is mostly a bash script.  With the former, it is rather difficult to merge changes in the sub-repository upstream. This is handled by the `split` command in the git-subtree extension, which makes it feasible. There simply isn't an elegant solution out there for our purpose, and subtree seems to be the best compromise so far given our goals.

## Merge direction
As a result of the complexity of merging to upstream, we prefer working on dependencies in their own repo, and update the parent repo by pulling. Merging upstream is possible but not encouraged.

## Handle git history
We decide to use `--squash` when merging to keep most of the dependency history out of the parent project. This cuts down noise in the parent repo.
We also choose not to use the `--rejoin` option of git-subtree, because this option creates an extra entry in git log to mark the current split location. This introduces noisy into our history. Without this option, `git subtree split` by default scans the entire history of the parent project, which can take a while if there has been a long history. To speed things up, one can provide a range of SHA to scan from, which usually means git only has to go through the last few changes.
