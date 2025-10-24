# Slightly weird Dockerfile, because we're starting from pre-baked artifacts. First, we're building
# one image tag per Postgres version, like '.../access:pg15' or whatever. The image will be a
# multi-arch image, though, so there's more than one manifest per image. To get this, the source
# binaries will all be loaded:
ARG PG_VER
FROM scratch AS amd64
ARG PG_VER
ADD access-pg${PG_VER}-amd64.tar.gz /
FROM scratch AS arm64
ARG PG_VER
ADD access-pg${PG_VER}-arm64.tar.gz /

# then the TARGETPLATFORM (which will look like 'linux/amd64') gets the 'linux/' prefix stripped
# off, and the build for a particular image only uses one of the source artifacts.
FROM ${TARGETPLATFORM#"linux/"} AS origin
# Now, the files that we care about get copied into the final location.
FROM scratch
ARG PG_VER
COPY --from=origin /usr/share/postgresql/${PG_VER}/extension/ /share/extension
COPY --from=origin /usr/lib/postgresql/${PG_VER}/lib/access_pgrx.so /lib/
# This can be built with a multi-platform-aware builder like so:
#   docker buildx build --build-arg PG_VER=15 --platform linux/amd64,linux/arm64 -t ... .
