## Images
Docker images are currently hosted on [Docker hub](https://hub.docker.com/r/thinkingfish/pelikan/)

## Pull
To pull the docker image created from this Dockerfile, run:
```sh
docker pull thinkingfish/pelikan
```

The docker image hub also include previous versions, tagged by version number.

## Run
The image contains all the binaries built from Pelikan. To invoke a specific
binary with a specific config, you will need to:
- log into a running container, OR
- override the `CMD` variable

### Interactive
To enter a running container of the pelikan image:
```sh
docker run -it --entrypoint=/bin/bash thinkingfish/pelikan
```

From here you can run any command you want, for example:
```sh
pelikan@a9eabfea0cee:/pelikan$ pelikan_twemcache -v
Version: 0.1.1
```

To run a binary with a pre-installed config file, you can run:
```sh
pelikan@a9eabfea0cee:/pelikan$ pelikan_slimcache /etc/pelikan/slimcache.conf
```

And exit when you are done.

### Override CMD
You can also override `CMD` when launching the container:
```sh
docker run --name pelikan thinkingfish/pelikan pelikan_slimcache /etc/pelikan/slimcache.conf
```

And when done, kill the container:
```sh
docker kill pelikan
```

## Note

### docker daemon not found (OSX)
On Mac OSX, if docker commands fail and throw the following message:
```sh
Cannot connect to the Docker daemon. Is the docker daemon running on this host?
```

Add the following line to your `bash` start script (or equivalent):
```
eval $(docker-machine env default)
```

