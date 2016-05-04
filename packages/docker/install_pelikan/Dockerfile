FROM debian:jessie

# add our user and group first to make sure their IDs get assigned consistently, regardless of whatever dependencies get added
RUN groupadd -r pelikan && useradd -r -g pelikan pelikan

ENV PELIKAN_VERSION 0.1.1
ENV PELIKAN_URL https://github.com/twitter/pelikan/archive/0.1.1.tar.gz
ENV PELIKAN_SHA1 b2dc5796702f106d14a645155a7837c11c79e231

# build and install binaries, config
RUN BUILDDEPS='cmake curl gcc libc6-dev make' \
	&& set -x \
	&& apt-get update && apt-get install --no-install-recommends -y $BUILDDEPS \
	&& rm -rf /var/lib/apt/lists/* \
	&& curl --insecure -SL "$PELIKAN_URL" -o pelikan.tar.gz \
	&& echo "$PELIKAN_SHA1  pelikan.tar.gz" | sha1sum -c - \
	&& mkdir -p /usr/src/pelikan \
	&& tar -xzf pelikan.tar.gz -C /usr/src/pelikan --strip-components=1 \
	&& rm pelikan.tar.gz \
	&& cd /usr/src/pelikan \
	&& mkdir -p /etc/pelikan \
	&& cp -f config/* /etc/pelikan \
	&& mkdir -p _build && cd _build \
	&& cmake .. \
	&& make -j \
	&& make install \
	&& cd / && rm -rf /usr/src/pelikan \
	&& apt-get purge -y --auto-remove $BUILDDEPS

RUN mkdir /pelikan && chown pelikan:pelikan /pelikan
VOLUME ["/pelikan"]
WORKDIR /pelikan

USER pelikan
# data port:
EXPOSE 12321
# admin port:
EXPOSE 9999
CMD ["which", "pelikan_pingserver"]
