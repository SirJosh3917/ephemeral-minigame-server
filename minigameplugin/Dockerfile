FROM gradle:7.3 AS build
WORKDIR /src

COPY . .
RUN gradle build --no-daemon

FROM scratch
WORKDIR /
COPY --from=build /src/build/libs/minigameplugin-1.0-SNAPSHOT-all.jar minigameplugin.jar
