FROM gradle:7.3 AS build
WORKDIR /src

COPY . .
RUN gradle build --no-daemon

FROM scratch
WORKDIR /
COPY --from=build /src/build/libs/serverplugin-1.0-SNAPSHOT-all.jar serverplugin.jar
